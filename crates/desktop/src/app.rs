use crate::{
    coordinator::{Coordinator, Phase},
    worker_client::{Command, Event, Telemetry, Worker},
};
use bevy_egui::egui::{self, TextureHandle};
use std::{io::Write, path::PathBuf, time::Instant};
use world_core::{Stroke, Tool, World};
pub struct Desktop {
    pub sim: Coordinator,
    pub worker: Option<Worker>,
    pub telemetry: Telemetry,
    pub brain_view: crate::brain_view::BrainView,
    pub music: crate::music::Music,
    pub tutorial: crate::tutorial::Tutorial,
    pub color_demos: Option<crate::tutorial_demo::ColorDemos>,
    pub focused: bool,
    pub latency: f64,
    pub root: PathBuf,
    pub run_dir: PathBuf,
    pub tool: Tool,
    pub radius: f64,
    pub ground_radius: f64,
    pub ink_radius: f64,
    pub color: Option<[u8; 3]>,
    pub stroke: Option<Stroke>,
    pub pending_strokes: std::collections::VecDeque<(u64, Stroke)>,
    pub gesture: u64,
    pub committed_gesture: Option<u64>,
    pub stroke_dirty: bool,
    pub pending_undo: Option<bool>,
    pub spacing: f64,
    pub world_texture: Option<TextureHandle>,
    pub intro_terrain_texture: Option<TextureHandle>,
    pub vision_texture: Option<TextureHandle>,
    pub vision: Vec<u8>,
    pub vision_hash: String,
    pub sent_step: Option<u64>,
    pub visual_revision: u64,
    pub notice: String,
    pub edit_notice: Option<([f64; 2], String)>,
    pub zoom: f32,
    pub pan: egui::Vec2,
    pub help: bool,
    pub details: bool,
    pub last_frame: Instant,
    journal_start: Instant,
    pub trail: Vec<[f64; 2]>,
    pub trace: Vec<f32>,
    pub restore_world: Option<World>,
    pub restore_name: Option<String>,
    pub want_save: bool,
    pub saved: bool,
    pub saving: bool,
    journal: Option<std::fs::File>,
}
impl Desktop {
    pub fn new(root: PathBuf) -> Self {
        let sim = Coordinator::new();
        let run_name = format!("play-{}", &sim.epoch[..12]);
        let run_dir = root.join("runs").join(&run_name);
        let _ = std::fs::create_dir_all(&run_dir);
        let journal = std::fs::File::create(run_dir.join("events.jsonl")).ok();
        let mut app = Self {
            brain_view: crate::brain_view::BrainView::load(),
            music: crate::music::Music::new(),
            tutorial: crate::tutorial::Tutorial::default(),
            color_demos: None,
            focused: true,
            sim,
            worker: None,
            telemetry: Telemetry::default(),
            latency: 0.,
            root,
            run_dir,
            tool: Tool::Solid,
            radius: 8.,
            ground_radius: 8.,
            ink_radius: 24.,
            color: None,
            stroke: None,
            pending_strokes: std::collections::VecDeque::new(),
            gesture: 0,
            committed_gesture: None,
            stroke_dirty: false,
            pending_undo: None,
            spacing: 1.,
            world_texture: None,
            intro_terrain_texture: None,
            vision_texture: None,
            vision: vec![16; 128 * 96 * 3],
            vision_hash: String::new(),
            sent_step: None,
            visual_revision: u64::MAX,
            notice: String::new(),
            edit_notice: None,
            zoom: 0.7,
            pan: egui::Vec2::ZERO,
            help: false,
            details: false,
            last_frame: Instant::now(),
            journal_start: Instant::now(),
            trail: vec![],
            trace: vec![],
            restore_world: None,
            restore_name: None,
            want_save: false,
            saved: false,
            saving: false,
            journal,
        };
        match Worker::launch(&app.root, &run_name) {
            Ok(w) => app.worker = Some(w),
            Err(e) => app.sim.fault(e),
        }
        app
    }
    pub fn select_tool(&mut self, tool: Tool) {
        if self.tool.solid() {
            self.ground_radius = self.radius;
        } else {
            self.ink_radius = self.radius;
        }
        self.tool = tool;
        self.radius = if tool.solid() {
            self.ground_radius
        } else {
            self.ink_radius
        };
    }
    pub fn select_color(&mut self, color: [u8; 3]) {
        self.color = if self.color == Some(color) {
            None
        } else {
            Some(color)
        };
        self.select_tool(Tool::Ink);
    }
    pub fn log(&mut self, mut value: serde_json::Value) {
        value["wall_ms"] = (self.journal_start.elapsed().as_secs_f64() * 1000.).into();
        if let Some(f) = &mut self.journal {
            let _ = writeln!(f, "{value}");
        }
    }
    pub fn send(&mut self, cmd: Command) {
        if let Some(w) = &self.worker {
            if let Err(e) = w.send(cmd) {
                self.sim.fault(e);
            }
        } else {
            self.sim.fault("The neural worker is unavailable".into());
        }
    }
    pub fn update(&mut self) {
        let dt = self.last_frame.elapsed().as_secs_f64();
        self.last_frame = Instant::now();
        self.update_with_dt(dt);
    }
    pub(crate) fn update_with_dt(&mut self, dt: f64) {
        self.sim.advance(dt);
        loop {
            let event = self.worker.as_ref().and_then(|w| w.try_recv().ok());
            let Some(event) = event else { break };
            match event {
                Event::Loaded { profile, telemetry } => {
                    self.music.reset();
                    self.sim.loaded(profile);
                    self.telemetry = telemetry;
                    match crate::tutorial_demo::ColorDemos::load(
                        &self.sim.profile,
                        self.telemetry.anatomy_activity.len(),
                    ) {
                        Ok(demos) => self.color_demos = Some(demos),
                        Err(error) => self.log(
                            serde_json::json!({"event":"tutorial_demo_unavailable", "error":error}),
                        ),
                    }
                    // Establish the epoch before any inspection or save, including
                    // a save taken before the first play observation.
                    let name = self.restore_name.take().unwrap_or_else(|| "initial".into());
                    if self.restore_world.is_none() {
                        self.restore_world = Some(World::level(self.sim.world.level));
                    }
                    self.sim.reset();
                    self.send(Command::Restore {
                        epoch: self.sim.epoch.clone(),
                        name,
                    });
                    let _ = std::fs::write(
                        self.run_dir.join("initial.world.json"),
                        self.sim.world.snapshot(),
                    );
                    let manifest = serde_json::json!({"mode":"MaleCNS fixed controller","learning":false,"decisions_hz":25,"neural_steps":2,"physics_ticks":4,"profile_sha256":self.sim.profile,"sensor":world_core::SENSOR_PROFILE,"image_log":"rolling 256 exact RGB frames","platform":std::env::consts::OS,"epoch":self.sim.epoch});
                    let _ =
                        std::fs::write(self.run_dir.join("manifest.json"), manifest.to_string());
                }
                Event::Action {
                    reply,
                    telemetry,
                    ms,
                } => match self.sim.accept(&reply) {
                    Ok(true) => {
                        self.latency = ms;
                        self.music.accept(&reply, &telemetry);
                        self.telemetry = telemetry;
                        self.trace.push(reply.action.steer as f32);
                        if self.trace.len() > 96 {
                            self.trace.remove(0);
                        }
                        self.log(
                            serde_json::json!({"event":"action","reply":reply,"latency_ms":ms}),
                        );
                    }
                    Ok(false) => {}
                    Err(e) => self.sim.fault(e),
                },
                Event::Restored {
                    epoch,
                    profile,
                    tick,
                    step_id,
                    telemetry,
                } => {
                    if epoch != self.sim.epoch {
                        continue;
                    }
                    if profile != self.sim.profile {
                        self.sim.fault("Restored profile does not match".into());
                        continue;
                    }
                    if let Some(w) = self.restore_world.take() {
                        if w.tick != tick {
                            self.sim.fault("World and brain checkpoint disagree".into());
                            continue;
                        }
                        self.sim.restored(w, step_id);
                        self.brain_view.reset();
                        self.music.reset();
                        self.telemetry = telemetry;
                        self.visual_revision = u64::MAX;
                        self.vision_texture = None;
                        self.vision = self.sim.world.observe();
                        self.sent_step = None;
                        self.vision_hash.clear();
                        self.trail.clear();
                        self.trace.clear();
                        self.notice.clear();
                        self.edit_notice = None;
                        self.log(serde_json::json!({"event":"restore","epoch":epoch,"tick":tick}));
                    }
                }
                Event::Saved { name, tick } => {
                    self.saving = false;
                    if tick != self.sim.world.tick {
                        self.sim.fault("Checkpoint boundary mismatch".into());
                        continue;
                    }
                    match std::fs::write(
                        self.run_dir.join(format!("{name}.world.json")),
                        self.sim.world.snapshot(),
                    ) {
                        Ok(()) => {
                            self.saved = true;
                            self.notice = "World and neural state saved together.".into();
                        }
                        Err(e) => self.notice = e.to_string(),
                    }
                }
                Event::Fault(e) => {
                    self.saving = false;
                    self.sim.fault(e);
                }
            }
        }
        self.sim.advance(0.0);
        if self.sim.phase == Phase::Paused && self.restore_name.is_none() && self.focused {
            self.tutorial.advance(dt.min(0.25) as f32);
        }
        if self.sim.phase == Phase::Waiting && self.sim.since.elapsed().as_secs_f64() > 2.5 {
            self.sim
                .fault("The neural worker took too long. Reset to recover.".into());
        }
        if self.sim.phase == Phase::Paused {
            let mut batches: Vec<(u64, Stroke)> = self.pending_strokes.drain(..).collect();
            if self.stroke_dirty {
                if let Some(s) = self.stroke.as_mut() {
                    batches.push((self.gesture, s.clone()));
                    if let Some(last) = s.points.last().copied() {
                        s.points = vec![last];
                    }
                }
                self.stroke_dirty = false;
            }
            for (gesture, s) in batches {
                let continuation = self.committed_gesture == Some(gesture);
                match self.sim.world.edit_live(&s, continuation) {
                    Ok(n) if n > 0 => {
                        self.committed_gesture = Some(gesture);
                        self.log(serde_json::json!({"event":"live_stroke","gesture":gesture,"tool":s.tool,"points":s.points,"radius":s.radius,"color":s.color,"revision":self.sim.world.revision,"physics_tick":self.sim.world.tick}));
                        self.notice.clear();
                        self.edit_notice = None;
                        if self.sim.want_pause {
                            self.vision = self.sim.world.observe();
                            self.vision_texture = None;
                            self.sent_step = None;
                        }
                    }
                    Ok(_) => {}
                    Err(e) => self.edit_notice = s.points.last().copied().map(|at| (at, e)),
                }
            }
            if let Some(redo) = self.pending_undo.take() {
                self.undo(redo);
            }
            if self.want_save && !self.saving {
                self.want_save = false;
                self.saving = true;
                self.send(Command::Snapshot {
                    epoch: self.sim.epoch.clone(),
                    name: "bookmark".into(),
                });
            }
            if self.restore_name.is_some() && !self.saving {
                let name = self.restore_name.take().unwrap();
                self.sim.reset();
                self.send(Command::Restore {
                    epoch: self.sim.epoch.clone(),
                    name,
                });
            }
            // The first gesture joins the usual edit boundary before its first
            // observation. Later strokes never override an explicit pause.
            if self.sim.phase == Phase::Paused && !self.saving && self.tutorial.take_start() {
                self.sim.play();
            }
        }
        if !self.saving && self.restore_name.is_none() && !self.tutorial.active() {
            if let Some((r, rgb)) = self.sim.request() {
                self.vision_hash = r.rgb_sha256.clone();
                self.sent_step = Some(r.step_id);
                self.vision = rgb;
                self.vision_texture = None;
                let path = format!("frame-{:03}.rgb", r.step_id % 256);
                let _ = std::fs::write(self.run_dir.join(&path), &self.vision);
                self.log(serde_json::json!({"event":"observation","epoch":r.epoch,"step_id":r.step_id,"physics_tick":r.physics_tick,"world_revision":r.world_revision,"rgb_sha256":r.rgb_sha256,"path":path}));
                self.send(Command::Step(r));
            }
        }
        if self.sim.phase == Phase::Applying {
            let p = [self.sim.world.actor.x, self.sim.world.actor.y];
            if self
                .trail
                .last()
                .is_none_or(|v| (v[0] - p[0]).abs() + (v[1] - p[1]).abs() > 2.)
            {
                self.trail.push(p);
                if self.trail.len() > 30 {
                    self.trail.remove(0);
                }
            }
        }
        self.music.update(
            !self.sim.want_pause
                && matches!(
                    self.sim.phase,
                    Phase::Paused | Phase::Waiting | Phase::Applying
                )
                && self.sim.world.outcome == world_core::Outcome::Running
                && !self.tutorial.active()
                && self.restore_name.is_none()
                && !self.saving
                && self.focused,
        );
    }
    pub fn reset(&mut self, bookmark: bool) {
        if matches!(self.sim.phase, Phase::Loading | Phase::Restoring)
            || (self.restore_name.is_some() && self.sim.phase != Phase::Fault)
        {
            return;
        }
        let world = if bookmark {
            std::fs::read(self.run_dir.join("bookmark.world.json"))
                .map_err(|e| e.to_string())
                .and_then(|b| World::restore(&b))
        } else {
            // A failed level transition can be retried with R, even when its
            // restore request was still queued when the worker stopped.
            let level = self
                .restore_world
                .as_ref()
                .map_or(self.sim.world.level, |w| w.level);
            Ok(World::level(level))
        };
        let world = match world {
            Ok(world) => world,
            Err(error) => {
                self.notice = format!("Cannot restore bookmark: {error}");
                return;
            }
        };
        if bookmark || world.level > 0 {
            self.tutorial = crate::tutorial::Tutorial::for_level(1);
        } else if self.tutorial.active() {
            self.tutorial.skip();
        }
        self.queue_restore(world, if bookmark { "bookmark" } else { "initial" });
        if !bookmark {
            self.tutorial.rearm_start();
        }
    }
    pub fn load_level(&mut self, level: usize) {
        if level >= world_core::LEVELS.len()
            || self.saving
            || self.restore_name.is_some()
            || matches!(
                self.sim.phase,
                Phase::Loading | Phase::Restoring | Phase::Fault
            )
        {
            return;
        }
        self.queue_restore(World::level(level), "initial");
        self.tutorial = crate::tutorial::Tutorial::for_level(level);
        self.zoom = 0.7;
        self.pan = egui::Vec2::ZERO;
        self.help = false;
        self.details = false;
        self.select_tool(
            if world_core::LEVELS[level].ground_zones.is_empty() && level != 0 {
                Tool::Ink
            } else {
                Tool::Solid
            },
        );
        self.color = None;
        if let Some(demos) = &mut self.color_demos {
            demos.reset();
        }
        self.log(serde_json::json!({"event":"select_level","level":level+1,"name":world_core::LEVELS[level].name}));
    }
    pub(crate) fn queue_restore(&mut self, world: World, name: &str) {
        self.music.reset();
        self.tutorial.cancel_start();
        self.edit_notice = None;
        self.stroke = None;
        self.pending_strokes.clear();
        self.stroke_dirty = false;
        self.committed_gesture = None;
        self.pending_undo = None;
        self.want_save = false;
        self.sim.pause();
        if self.sim.phase == Phase::Fault {
            self.worker = None;
            let run_name = self.run_dir.file_name().unwrap().to_string_lossy();
            match Worker::launch(&self.root, &run_name) {
                Ok(w) => {
                    self.worker = Some(w);
                    self.sim.phase = Phase::Loading;
                    self.notice = "Reloading the neural controller.".into();
                }
                Err(e) => self.sim.fault(e),
            }
            self.restore_world = Some(world);
            self.restore_name = Some(name.into());
        } else if self.sim.phase != Phase::Loading && self.sim.phase != Phase::Restoring {
            self.restore_world = Some(world);
            self.restore_name = Some(name.into());
        }
    }
    pub fn toggle_play(&mut self) {
        if self.saving || self.tutorial.active() {
            return;
        }
        let pending_start = self.tutorial.take_start();
        self.tutorial.cancel_start();
        if pending_start {
            self.sim.pause();
            return;
        }
        if self.sim.want_pause {
            self.sim.play()
        } else {
            self.sim.pause()
        }
    }
    pub fn replay_tutorial(&mut self) {
        self.load_level(0);
    }
    pub fn undo(&mut self, redo: bool) {
        if self.sim.phase != Phase::Paused || self.saving {
            self.pending_undo = Some(redo);
            return;
        }
        self.committed_gesture = None;
        let source = if redo {
            self.sim.world.future.last()
        } else {
            self.sim.world.history.last()
        }
        .cloned();
        let revision = self.sim.world.revision;
        let result = if redo {
            self.sim.world.redo()
        } else {
            self.sim.world.undo()
        };
        if result.is_ok() {
            if let Some(mut delta) = source {
                delta.based_on_revision = revision;
                if !redo {
                    for c in &mut delta.changes {
                        std::mem::swap(&mut c.before, &mut c.after);
                    }
                }
                self.log(serde_json::json!({"event":"edit","operation":if redo {"redo"} else {"undo"},"revision":self.sim.world.revision,"delta":delta}));
            }
        }
        self.notice = result
            .map(|_| if redo { "Redone." } else { "Undone." }.to_string())
            .unwrap_or_else(|e| e);
    }
    pub fn textures(&mut self, ctx: &egui::Context) {
        if self.intro_terrain_texture.is_none() {
            self.intro_terrain_texture = Some(ctx.load_texture(
                "intro-terrain",
                egui::ColorImage::from_rgb([640, 360], &World::bridge().compose_without_goal()),
                egui::TextureOptions::NEAREST,
            ));
        }
        if let Some(demos) = &mut self.color_demos {
            demos.textures(ctx);
        }
        if self.world_texture.is_none() || self.visual_revision != self.sim.world.revision {
            let image = egui::ColorImage::from_rgb([640, 360], &self.sim.world.compose());
            if let Some(t) = self.world_texture.as_mut() {
                t.set(image, egui::TextureOptions::NEAREST)
            } else {
                self.world_texture =
                    Some(ctx.load_texture("world", image, egui::TextureOptions::NEAREST));
            }
            self.visual_revision = self.sim.world.revision;
        }
        if self.vision_texture.is_none() {
            self.vision_texture = Some(ctx.load_texture(
                "actual-observation",
                egui::ColorImage::from_rgb([128, 96], &self.vision),
                egui::TextureOptions::NEAREST,
            ));
        }
    }
}
