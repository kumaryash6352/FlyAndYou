use crate::{
    app::Desktop,
    coordinator::Phase,
    trailer_scene::{self as scene, Scene},
    tutorial::Tutorial,
};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use world_core::{Outcome, Stroke};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Beat {
    Opening,
    Premise,
    Construction,
    Persuasion,
    Payoff,
    Red,
    Rug,
    Wall,
    End,
}

pub struct Trailer {
    pub app: Desktop,
    pub beat: Beat,
    pub elapsed: f64,
    pub total: f64,
    pub started: bool,
    pub paused: bool,
    pub finished: bool,
    pub error: String,
    pub cursor: Option<([f64; 2], Stroke)>,
    pub saw_approach: bool,
    pub saw_retreat: bool,
    pub peak_speed: f64,
    pub autostart: bool,
    transition: Option<Beat>,
    brush_progress: f64,
    motion_started: bool,
    last_frame: Instant,
}

impl Trailer {
    pub fn new(root: PathBuf, autostart: bool) -> Self {
        let mut app = Desktop::new(root);
        app.tutorial = Tutorial::for_level(1);
        app.log(serde_json::json!({"event":"trailer_open","automatic_strokes":true}));
        Self {
            app,
            beat: Beat::Opening,
            elapsed: 0.,
            total: 0.,
            started: false,
            paused: false,
            finished: false,
            error: String::new(),
            cursor: None,
            saw_approach: false,
            saw_retreat: false,
            peak_speed: 0.,
            autostart,
            transition: None,
            brush_progress: 0.,
            motion_started: false,
            last_frame: Instant::now(),
        }
    }
    pub fn ready(&self) -> bool {
        self.app.sim.phase == Phase::Paused
            && self.app.restore_name.is_none()
            && self.app.restore_world.is_none()
            && !self.app.saving
    }
    pub fn start(&mut self) {
        if !self.started && self.ready() {
            self.started = true;
            self.last_frame = Instant::now();
            self.app.log(serde_json::json!({"event":"trailer_start"}));
        }
    }
    pub fn restart(&mut self) {
        if matches!(self.app.sim.phase, Phase::Loading | Phase::Restoring) {
            return;
        }
        let _ = std::fs::remove_file(self.app.run_dir.join("trailer-verification.json"));
        self.app
            .queue_restore(scene::world(Scene::Story), "initial");
        self.transition = None;
        self.beat = Beat::Opening;
        self.elapsed = 0.;
        self.total = 0.;
        self.started = false;
        self.finished = false;
        self.paused = false;
        self.error.clear();
        self.cursor = None;
        self.saw_approach = false;
        self.saw_retreat = false;
        self.peak_speed = 0.;
        self.motion_started = false;
        self.brush_progress = 0.;
    }
    fn next(&mut self, beat: Beat) {
        self.beat = beat;
        self.elapsed = 0.;
        self.brush_progress = 0.;
        self.cursor = None;
        self.app.log(serde_json::json!({"event":"trailer_beat","beat":format!("{beat:?}"),"seconds":self.total,
            "outcome":format!("{:?}",self.app.sim.world.outcome),"approach_seen":self.saw_approach}));
    }
    fn cut(&mut self, beat: Beat, scene: Scene) {
        self.app.queue_restore(scene::world(scene), "initial");
        self.transition = Some(beat);
        self.motion_started = false;
        self.cursor = None;
    }
    fn draw_stroke(&mut self, stroke: Stroke, progress: f64) {
        let progress = progress.clamp(0., 1.);
        if progress <= self.brush_progress {
            return;
        }
        if self.brush_progress == 0. {
            self.app.gesture += 1;
        }
        let fragment = scene::fragment(&stroke, self.brush_progress, progress);
        self.cursor = Some((*fragment.points.last().unwrap(), stroke));
        self.app
            .pending_strokes
            .push_back((self.app.gesture, fragment));
        self.brush_progress = progress;
    }
    fn move_fly(&mut self) {
        if !self.motion_started && self.ready() {
            self.app.sim.play();
            self.motion_started = true;
        }
    }
    pub fn update(&mut self) {
        let dt = self.last_frame.elapsed().as_secs_f64().min(0.1);
        self.last_frame = Instant::now();
        let playing = self.started && !self.paused && self.app.focused && self.error.is_empty();
        if !playing {
            self.app.sim.pause();
        } else if self.motion_started && self.ready() {
            self.app.sim.play();
        }
        self.app.update_with_dt(dt);
        if self.app.sim.phase == Phase::Fault {
            return;
        }
        if self.autostart && !self.started {
            self.start();
        }
        if !playing {
            return;
        }
        if let Some(beat) = self.transition {
            if self.ready() {
                self.transition = None;
                self.next(beat);
                self.move_fly();
            }
            return;
        }
        self.elapsed += dt;
        self.total += dt;
        let w = &self.app.sim.world;
        if self.beat == Beat::Persuasion {
            self.peak_speed = self.peak_speed.max(w.actor.vx);
            self.saw_approach |= self.app.telemetry.motor_mode == "approach"
                && w.actor.vx > 110.
                && w.actor.x > 225.;
        }
        if self.beat == Beat::Red {
            self.saw_retreat |= self.app.telemetry.motor_mode == "retreat" && w.actor.vx < -80.;
        }
        if matches!(self.beat, Beat::Construction | Beat::Persuasion)
            && w.outcome == Outcome::Failed
        {
            self.fail("Fly fell during the take. Press R to reset and rehearse again.");
            return;
        }
        match self.beat {
            Beat::Opening if self.elapsed >= 14. => self.next(Beat::Premise),
            Beat::Premise if self.elapsed >= 7. => self.next(Beat::Construction),
            Beat::Construction => {
                if self.elapsed >= 4.8 {
                    self.draw_stroke(scene::bridge_stroke(), (self.elapsed - 4.8) / 1.1);
                    self.move_fly();
                }
                if self.elapsed >= 6. {
                    self.next(Beat::Persuasion);
                }
            }
            Beat::Persuasion => {
                self.draw_stroke(scene::yellow_stroke(), self.elapsed / 1.5);
                if self.elapsed > 2. {
                    self.cursor = None;
                }
                if self.elapsed >= 8.
                    && scene::payoff_ready(self.app.sim.world.outcome, self.saw_approach)
                {
                    self.motion_started = false;
                    self.next(Beat::Payoff);
                } else if self.elapsed > 20. {
                    self.fail("The take did not produce a visible approach and a real win. Press R to retry.");
                }
            }
            Beat::Payoff if self.elapsed >= 5. => self.cut(Beat::Red, Scene::Red),
            Beat::Red => {
                self.draw_stroke(scene::red_stroke(), (self.elapsed - 0.25) / 0.5);
                if self.elapsed > 1.1 {
                    self.cursor = None;
                }
                if self.elapsed >= 2.4 && self.saw_retreat {
                    self.cut(Beat::Rug, Scene::Rug);
                } else if self.elapsed > 6. {
                    self.fail("The red cue did not produce a visible retreat. Press R to retry.");
                }
            }
            Beat::Rug => {
                self.draw_stroke(scene::rug_stroke(), (self.elapsed - 0.1) / 0.4);
                if self.elapsed > 0.8 {
                    self.cursor = None;
                }
                if self.elapsed >= 2.2 {
                    self.cut(Beat::Wall, Scene::Wall);
                }
            }
            Beat::Wall => {
                self.draw_stroke(
                    scene::tower(world_core::Tool::EraseSolid),
                    (self.elapsed - 1.1) / 0.45,
                );
                if self.elapsed > 1.8 {
                    self.cursor = None;
                }
                if self.elapsed >= 2.4 {
                    self.app.sim.pause();
                    self.motion_started = false;
                    self.next(Beat::End);
                }
            }
            Beat::End if !self.finished && self.total >= 60. && self.elapsed >= 7. => {
                self.finished = true;
                let evidence = serde_json::json!({"event":"trailer_complete","seconds":self.total,
                    "real_win":true,"approach_seen":self.saw_approach,"retreat_seen":self.saw_retreat,
                    "peak_forward_speed":self.peak_speed,"profile":self.app.sim.profile});
                self.app.log(evidence.clone());
                let _ = std::fs::write(
                    self.app.run_dir.join("trailer-verification.json"),
                    evidence.to_string(),
                );
            }
            _ => {}
        }
    }
    fn fail(&mut self, message: &str) {
        self.error = message.into();
        self.app.sim.pause();
        self.app
            .log(serde_json::json!({"event":"trailer_failed","error":message}));
    }
}

pub fn run(root: PathBuf) {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help") {
        println!(
            "fly-trailer [--autostart | --rehearse]\nEnter: start · Space: pause · R: reset take\n--rehearse runs the complete real-controller take without a window and checks its outcome."
        );
        return;
    }
    if args.iter().any(|a| a != "--autostart" && a != "--rehearse") {
        eprintln!("Unknown argument. Use --help.");
        std::process::exit(2);
    }
    let rehearse = args.iter().any(|a| a == "--rehearse");
    if rehearse {
        let mut trailer = Trailer::new(root, true);
        println!(
            "Rehearsing with the full controller; evidence: {}",
            trailer.app.run_dir.display()
        );
        let since = Instant::now();
        while !trailer.finished
            && trailer.error.is_empty()
            && trailer.app.sim.phase != Phase::Fault
            && since.elapsed().as_secs() < 150
        {
            trailer.update();
            std::thread::sleep(Duration::from_millis(4));
        }
        if trailer.finished {
            println!(
                "PASS: {:.2}s, real win, yellow approach ({:.1} px/s), red retreat",
                trailer.total, trailer.peak_speed
            );
        } else {
            eprintln!("FAIL: {} {}", trailer.error, trailer.app.sim.error);
            drop(trailer);
            std::process::exit(1);
        }
        return;
    }
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "fly & you — trailer".into(),
                        resolution: (1280, 720).into(),
                        resize_constraints: WindowResizeConstraints {
                            min_width: 960.,
                            min_height: 540.,
                            ..default()
                        },
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(EguiPlugin {
            bindless_mode_array_size: None,
            ..default()
        })
        .insert_non_send(TrailerHost {
            root: Some(root),
            autostart: args.iter().any(|a| a == "--autostart"),
            trailer: None,
            loading: None,
            painted: false,
            error: String::new(),
        })
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Camera2d);
        })
        .add_systems(Update, |mut host: NonSendMut<TrailerHost>| host.update())
        .add_systems(EguiPrimaryContextPass, draw)
        .run();
}
struct TrailerHost {
    root: Option<PathBuf>,
    autostart: bool,
    trailer: Option<Trailer>,
    loading: Option<std::sync::mpsc::Receiver<Trailer>>,
    painted: bool,
    error: String,
}
impl TrailerHost {
    fn update(&mut self) {
        if self.painted
            && let Some(root) = self.root.take()
        {
            let (tx, rx) = std::sync::mpsc::channel();
            self.loading = Some(rx);
            let autostart = self.autostart;
            // macOS may wait here for Documents consent. Keep the window responsive.
            std::thread::spawn(move || {
                let _ = tx.send(Trailer::new(root, autostart));
            });
        }
        if let Some(rx) = &self.loading {
            match rx.try_recv() {
                Ok(trailer) => {
                    self.trailer = Some(trailer);
                    self.loading = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.error =
                        "Loading stopped. Close this window and launch the trailer again.".into();
                    self.loading = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(trailer) = &mut self.trailer {
            trailer.update();
        }
    }
}
fn draw(
    mut contexts: EguiContexts,
    mut host: NonSendMut<TrailerHost>,
    mut configured: Local<bool>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    if !*configured {
        crate::display::configure(ctx);
        *configured = true;
    }
    if let Some(trailer) = &mut host.trailer {
        crate::trailer_view::draw(trailer, ctx);
    } else {
        crate::trailer_view::loading(ctx, &host.error);
        host.painted = true;
    }
    Ok(())
}
