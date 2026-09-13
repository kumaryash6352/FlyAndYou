use crate::{app::Desktop, coordinator::Phase, tutorial_view};
use bevy_egui::egui::*;
use world_core::{Outcome, Stroke as PaintStroke, Tool};
const PAPER: Color32 = Color32::from_rgb(244, 241, 230);
const INK: Color32 = Color32::from_rgb(42, 48, 38);
const MUTED: Color32 = Color32::from_rgb(116, 123, 104);
const LINE: Color32 = Color32::from_rgb(213, 215, 197);
const GREEN: Color32 = Color32::from_rgb(82, 104, 60);
const OUTSIDE_TERRAIN: Color32 = Color32::from_rgb(119, 127, 110);
pub fn configure(ctx: &Context) {
    tutorial_view::install_font(ctx);
    let mut style = Style::default();
    style.visuals = Visuals::dark();
    style.visuals.window_fill = Color32::from_rgba_unmultiplied(21, 24, 25, 242);
    style.visuals.override_text_color = Some(Color32::from_rgb(221, 226, 216));
    style.visuals.selection.bg_fill = GREEN;
    style.spacing.button_padding = vec2(7., 4.);
    style.spacing.item_spacing = vec2(5., 5.);
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(12.));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(12.));
    ctx.set_global_style(style);
}
fn button(ui: &mut Ui, s: &str, selected: bool) -> Response {
    ui.add(Button::new(s).selected(selected))
}
pub(crate) fn fly(p: &Painter, c: Pos2, scale: f32, tick: u64, _vx: f64, falling: bool) {
    let flap = (tick / 3) % 3;
    let leg = (tick / 5) % 2;
    let shapes = [
        (-4., 1., 6., 5.),
        (1., 0., 4., 4.),
        (4., 1., 2., 3.),
        (-5., 2., 2., 3.),
    ];
    let ink = Color32::from_rgb(26, 30, 24);
    // Tiny wings and legs, deliberately drawn as coarse pixels.
    for side in [-1., 1.] {
        let yy = if flap == 0 {
            -6.
        } else if flap == 1 {
            -4.
        } else {
            -2.
        };
        let x = -1. + side * 2.;
        let r = Rect::from_min_size(
            c + vec2(x * scale, yy * scale),
            vec2(3. * scale, 5. * scale),
        );
        p.rect_filled(r, 0, Color32::from_rgb(201, 211, 190));
        p.line_segment(
            [r.left_top(), r.right_bottom()],
            Stroke::new(scale * 0.6, Color32::from_rgb(123, 141, 116)),
        );
    }
    for (x, y, w, h) in shapes {
        p.rect_filled(
            Rect::from_min_size(c + vec2(x * scale, y * scale), vec2(w * scale, h * scale)),
            0,
            ink,
        );
    }
    for i in 0..3 {
        let x = (i as f32 * 3. - 3.) * scale;
        let shift = if leg == 0 { 1. } else { -1. };
        let end = c + vec2(x + (if falling { 2. } else { shift }) * scale, 8. * scale);
        p.line_segment([c + vec2(x, 4. * scale), end], Stroke::new(scale, ink));
        p.line_segment([end, end + vec2(-1.5 * scale, 0.)], Stroke::new(scale, ink));
    }
    p.rect_filled(
        Rect::from_min_size(c + vec2(4. * scale, 1. * scale), vec2(scale, scale)),
        0,
        Color32::from_rgb(103, 68, 49),
    );
}
impl Desktop {
    pub fn ui(&mut self, ctx: &Context) {
        self.textures(ctx);
        let first = ctx.current_pass_index() == 0;
        self.focused = ctx.input(|i| i.focused);
        if first {
            ctx.input(|i| {
                if self.tutorial.awaiting_start() {
                    let enter = i.events.iter().any(|event| {
                        matches!(
                            event,
                            Event::Key {
                                key: Key::Enter,
                                pressed: true,
                                repeat: false,
                                ..
                            }
                        )
                    });
                    if enter
                        && i.focused
                        && self.sim.phase == Phase::Paused
                        && self.restore_name.is_none()
                        && !self.saving
                    {
                        self.tutorial.start_intro();
                    }
                    if i.key_pressed(Key::R) && self.sim.phase == Phase::Fault {
                        self.reset(false);
                    }
                    return;
                }
                if i.key_pressed(Key::N)
                    && !self.tutorial.active()
                    && self.sim.world.outcome == Outcome::Won
                {
                    self.load_level(self.sim.world.level + 1);
                }
                if i.key_pressed(Key::Space) {
                    self.toggle_play();
                }
                if i.key_pressed(Key::R) && !i.modifiers.command {
                    self.reset(false);
                }
                if i.key_pressed(Key::F5) && !self.tutorial.active() {
                    self.sim.pause();
                    self.want_save = true;
                }
                if i.key_pressed(Key::F9) && !self.tutorial.active() {
                    self.reset(true);
                }
                if i.key_pressed(Key::H) {
                    self.help = !self.help;
                }
                for (key, tool) in [
                    (Key::Num1, Tool::Solid),
                    (Key::Num2, Tool::Ink),
                    (Key::Num3, Tool::EraseSolid),
                    (Key::Num4, Tool::EraseInk),
                ] {
                    if i.key_pressed(key) && self.tutorial.show_tools() {
                        self.select_tool(tool);
                    }
                }
                if i.modifiers.command && i.key_pressed(Key::Z) && !self.tutorial.active() {
                    self.undo(i.modifiers.shift);
                }
                if i.key_pressed(Key::Escape) || !i.focused {
                    self.stroke = None;
                    self.stroke_dirty = false;
                }
            });
        }
        let mut viewport = Ui::new(
            ctx.clone(),
            Id::new("fly-root"),
            UiBuilder::new()
                .layer_id(LayerId::background())
                .max_rect(ctx.content_rect()),
        );
        CentralPanel::default()
            .frame(Frame::NONE)
            .show(&mut viewport, |ui| {
                let all = ui.max_rect();
                let width = (all.width() * 0.22).clamp(240., 360.);
                ui.painter().rect_filled(all, 0, tutorial_view::PAPER);
                let split = all.right() - width * self.tutorial.rail_fraction();
                let game = Rect::from_min_max(all.min, pos2(split - 1., all.bottom()));
                let rail = Rect::from_min_max(pos2(split, all.top()), all.max);
                let response = ui.interact(game, Id::new("game-canvas"), Sense::click_and_drag());
                self.canvas(ui, ctx, game, response, first);
                if !self.tutorial.show_rail() || self.tutorial.rail_fraction() < 0.03 {
                    return;
                }
                let brain = Rect::from_min_max(
                    rail.min,
                    pos2(rail.right(), rail.top() + rail.height() * 0.59),
                );
                if let Some(index) = self.tutorial.demo()
                    && let Some(demos) = &mut self.color_demos
                {
                    demos.draw_brain(ui, brain, index, self.tutorial.elapsed);
                } else {
                    self.brain_view.draw(
                        ui,
                        brain,
                        self.telemetry.ticks,
                        &self.telemetry.anatomy_activity,
                    );
                }
                let eye = Rect::from_min_max(pos2(rail.left(), brain.bottom() + 1.), rail.max);
                let eye_texture = self
                    .tutorial
                    .demo()
                    .and_then(|i| self.color_demos.as_ref()?.clips[i].eye_texture.as_ref())
                    .or(self.vision_texture.as_ref());
                if let Some(t) = eye_texture {
                    ui.painter()
                        .rect_filled(eye, 0, Color32::from_rgb(21, 24, 25));
                    let size =
                        vec2(128., 96.) * (eye.width() / 128.).min((eye.height() - 38.) / 96.);
                    ui.painter().image(
                        t.id(),
                        Rect::from_center_size(eye.center() + vec2(0., 15.), size),
                        Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                        Color32::WHITE,
                    );
                }
                tutorial_view::lettering(
                    ui,
                    Rect::from_min_size(eye.min + vec2(6., 4.), vec2(eye.width() - 12., 32.)),
                    "FlyCam™",
                    22.,
                    Color32::from_rgb(224, 225, 211),
                );
                ui.interact(eye, Id::new("fly-eye"), Sense::hover())
                    .on_hover_text(if self.tutorial.demo().is_some() && self.color_demos.is_some() {
                        "Color demonstration: this image drove the recorded model response above."
                    } else if self.sent_step.is_some() {
                        "The exact image sent to the fly."
                    } else {
                        "Its next look."
                    });
            });
        if self.tutorial.show_tools() || self.sim.phase == Phase::Fault {
            self.toolbox(ctx, first);
        }
        if self.tutorial.active() && self.tutorial.beat > 0 && !self.tutorial.can_paint() {
            Area::new(Id::new("skip-intro"))
                .anchor(Align2::LEFT_BOTTOM, vec2(14., -14.))
                .show(ctx, |ui| {
                    if ui.small_button("Skip intro").clicked() && first {
                        self.tutorial.skip();
                    }
                });
        }
    }
    fn toolbox(&mut self, ctx: &Context, first: bool) {
        Window::new("fly & you").default_pos(pos2(10.,10.)).default_width(292.)
            .resizable(false).collapsible(true).show(ctx, |ui| {
                let ready = !matches!(self.sim.phase, Phase::Loading | Phase::Restoring | Phase::Fault) && !self.saving && self.restore_name.is_none();
                if !self.tutorial.active() {
                    let mut level = self.sim.world.level;
                    ui.add_enabled_ui(ready, |ui| {
                        ComboBox::from_id_salt("campaign-level")
                            .selected_text(format!("{} / 6   {}", level + 1, world_core::LEVELS[level].name))
                            .show_ui(ui, |ui| {
                                for (index, spec) in world_core::LEVELS.iter().enumerate() {
                                    ui.selectable_value(&mut level, index, format!("{}   {}", index + 1, spec.name));
                                }
                            });
                    });
                    if first && level != self.sim.world.level { self.load_level(level); }
                    if self.sim.world.outcome == Outcome::Won {
                        if self.sim.world.level + 1 < world_core::LEVELS.len() {
                            if ui.add_enabled(ready, Button::new("Next level  [N]")).clicked() && first {
                                self.load_level(self.sim.world.level + 1);
                            }
                        } else { ui.label("Six levels. One very lucky fly."); }
                    }
                }
                ui.horizontal(|ui| {
                    if ui.add_enabled(ready && !self.tutorial.active() && self.sim.world.outcome == Outcome::Running,
                        Button::new(if self.sim.want_pause { "Run  [Space]" } else { "Pause  [Space]" })).clicked() && first { self.toggle_play(); }
                    if ui.button("Reset  [R]").clicked() && first { self.reset(false); }
                    if ui.add_enabled(ready && !self.tutorial.active() && !self.sim.world.history.is_empty(), Button::new("Undo")).clicked() && first { self.undo(false); }
                });
                ui.horizontal(|ui| {
                    for (tool,label) in [(Tool::Solid,"1 Ground"),(Tool::Ink,"2 Ink"),(Tool::EraseSolid,"3 Erase ground"),(Tool::EraseInk,"4 Erase ink")] {
                        if button(ui,label,self.tool == tool).clicked() && first { self.select_tool(tool); }
                    }
                });
                ui.horizontal(|ui| {
                    for c in [[232,186,60],[195,80,57],[94,132,164],[92,119,72],[43,46,39]] {
                        let (r,res) = ui.allocate_exact_size(vec2(19.,19.),Sense::click());
                        ui.painter().rect_filled(r.shrink(2.),0,Color32::from_rgb(c[0],c[1],c[2]));
                        if self.color == Some(c) { ui.painter().rect_stroke(r,0,Stroke::new(1.,Color32::WHITE),StrokeKind::Inside); }
                        if res.clicked() && first { self.select_color(c); }
                        res.on_hover_text(match c {
                            [232,186,60] => "Yellow",
                            [195,80,57] => "Red",
                            [94,132,164] => "Blue",
                            [92,119,72] => "Green",
                            _ => "Black",
                        });
                    }
                    if button(ui, "×", self.color.is_none()).on_hover_text("No color. Ground stays neutral; Ink needs a color. Click a selected swatch again to clear it.").clicked() && first { self.color = None; }
                    ui.add(Slider::new(&mut self.radius, if self.tool.solid() { 4.0..=24.0 } else { 1.0..=24.0 }).show_value(false).text("size"));
                });
                ui.horizontal(|ui| {
                    if button(ui, "Controls / H", self.help).clicked() && first { self.help = !self.help; }
                    if ui.add_enabled(!self.tutorial.active(), Button::new(if self.zoom < 1. { "Follow" } else { "Whole map" }).small()).clicked() && first {
                        self.zoom = if self.zoom < 1. { 1. } else { 0.7 }; self.pan = Vec2::ZERO;
                    }
                    if button(ui, "Brain", self.details).clicked() && first { self.details = !self.details; }
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.music.enabled, "Music")
                        .on_hover_text("Live modeled brain activity shapes soft notes, buzz, and little taps. Yellow warms the sound; red adds tension. Pausing lets it fade.");
                    ui.add_enabled(self.music.enabled, Slider::new(&mut self.music.volume, 0.0..=1.0)
                        .show_value(false).text("volume"));
                });
                if let Some(error) = self.music.error().map(str::to_owned) {
                    if ui.small_button("Retry music").on_hover_text(error).clicked() && first {
                        self.music.open_output();
                    }
                }
                if self.help {
                    ui.separator();
                    ui.label("Drag: draw · Space: run / pause · 1–4: tools");
                    ui.label("Color → Ink. Click again or × to clear.");
                    ui.label("Ground uses your color. Empty-space Ink colors both side walls.");
                    ui.label("Cmd/Ctrl Z: undo · Shift Cmd/Ctrl Z: redo");
                    ui.label("R: reset · F5 / F9: save / restore · N: next level");
                    if ui.add_enabled(ready && !self.tutorial.active(), Button::new("Replay intro")).clicked() && first { self.replay_tutorial(); }
                    ui.collapsing("Credits / licenses", |ui| {
                        ui.label("MaleCNS v1.0 connectivity and anatomy: FlyEM / HHMI Janelia, University of Cambridge, MRC Laboratory of Molecular Biology, and Google Research.");
                        ui.hyperlink_to("MaleCNS dataset", "https://male-cns.janelia.org/download/");
                        ui.hyperlink_to("CC BY 4.0", "https://creativecommons.org/licenses/by/4.0/");
                        ui.label("The game filters and normalizes the graph, approximates transmitter signs, and adds artificial visual and motor mappings. The sampled anatomy display and neural activity are transformed for the game.");
                        ScrollArea::vertical().id_salt("credits").max_height(180.).show(ui, |ui| {
                            ui.label(crate::THIRD_PARTY_NOTICES);
                            ui.label(crate::CANDLE_LICENSE);
                        });
                    });
                }
                if self.details {
                    ui.separator();
                    if self.sim.world.outcome == Outcome::Running {
                        let intent = match self.telemetry.motor_mode.as_str() {
                            "approach" => "Approaching yellow",
                            "retreat" => "Retreating from red",
                            "hold" => "Holding near red",
                            "search" => "Searching",
                            _ => "",
                        };
                        if !intent.is_empty() {
                            let direction = if self.telemetry.steer == 0. { "Still" }
                                else if self.telemetry.heading < 0 { "Left" } else { "Right" };
                            ui.label(RichText::new(format!("{direction}: {intent}")).weak())
                                .on_hover_text("Current neural motor output.");
                        }
                    }

                    ui.label(format!("{} neurons · {} connections",self.telemetry.nodes,self.telemetry.edges));
                    ui.label(format!("{:.0} ms / decision · learning off", self.latency));
                    if let (Some(approach), Some(avoidance)) = (self.telemetry.approach, self.telemetry.avoidance) {
                        if approach.is_finite() && avoidance.is_finite() {
                            ui.label(format!("Yellow pull {approach:.2} · Red push {avoidance:.2}"))
                                .on_hover_text("Neural activity above a fixed neutral baseline. These are signal strengths, not probabilities.");
                        }
                    } else if !self.telemetry.motor_mode.is_empty() {
                        ui.label("Neural color evidence unavailable");
                    }
                    ui.label("Actual sampled anatomy. Orange shows activity above recent baseline; blue shows structure.");
                }
                if self.sim.phase == Phase::Fault { ui.colored_label(Color32::LIGHT_RED, &self.sim.error); }
                else if !ready { ui.label(self.sim.label()); }
                else if !self.notice.is_empty() { ui.label(RichText::new(&self.notice).weak()); }
            });
    }
    fn canvas(&mut self, ui: &mut Ui, ctx: &Context, area: Rect, response: Response, first: bool) {
        let painter = ui.painter().with_clip_rect(area);
        painter.rect_filled(area, 0, OUTSIDE_TERRAIN);
        let fit = (area.width() / 640.).min(area.height() / 360.);
        let cover = (area.width() / 640.).max(area.height() / 360.);
        let scale = if self.zoom < 1. {
            fit
        } else {
            cover * self.zoom
        };
        let follow = (self.sim.world.actor.x as f32 * scale - area.width() * 0.35)
            .clamp(0., (640. * scale - area.width()).max(0.));
        let rect = if self.zoom < 1. {
            Rect::from_center_size(area.center() + self.pan, vec2(640. * scale, 360. * scale))
        } else {
            Rect::from_min_size(
                area.min + self.pan - vec2(follow, 0.),
                vec2(640. * scale, 360. * scale),
            )
        };
        let (rect, scale) = if self.tutorial.active() {
            let rect = tutorial_view::map_rect(area, &self.tutorial);
            (rect, rect.width() / 640.)
        } else {
            (rect, scale)
        };
        // Extend the spectator scenery beyond the map. Anchor its hatching
        // to world coordinates so it stays still during pans and zooms.
        let spacing = 17. * scale;
        let origin = rect.left() + 2. * rect.top();
        let first_line = ((area.left() + 2. * area.top() - origin) / spacing).floor() as i32;
        let last_line = ((area.right() + 2. * area.bottom() - origin) / spacing).ceil() as i32;
        for line in first_line..=last_line {
            let intercept = origin + line as f32 * spacing;
            painter.line_segment(
                [
                    pos2(intercept - 2. * area.top(), area.top()),
                    pos2(intercept - 2. * area.bottom(), area.bottom()),
                ],
                Stroke::new(scale * 0.7, Color32::from_rgb(111, 119, 103)),
            );
        }
        let world_texture = if self.tutorial.beat < 4 {
            self.intro_terrain_texture.as_ref()
        } else {
            self.tutorial
                .demo()
                .and_then(|i| self.color_demos.as_ref()?.clips[i].world_texture.as_ref())
                .or(self.world_texture.as_ref())
        };
        if let Some(t) = world_texture {
            painter.image(
                t.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                Color32::WHITE,
            );
        }
        if !self.tutorial.active() {
            crate::campaign_view::draw(&painter, rect, scale, &self.sim.world);
            if let Some((at, message)) = &self.edit_notice {
                crate::campaign_view::edit_feedback(&painter, rect, scale, *at, message);
            }
        }
        if !self.tutorial.active() && ctx.input(|i| i.key_down(Key::C)) {
            for y in 0..90 {
                for x in 0..160 {
                    if self.sim.world.occupied(x, y) {
                        let r = Rect::from_min_size(
                            rect.min + vec2(x as f32 * 4. * scale, y as f32 * 4. * scale),
                            vec2(4. * scale, 4. * scale),
                        );
                        painter.rect_stroke(
                            r,
                            0,
                            Stroke::new(0.5, Color32::from_rgb(190, 88, 56)),
                            StrokeKind::Inside,
                        );
                    }
                }
            }
        }
        let actor = &self.sim.world.actor;
        let center = rect.min + vec2(actor.x as f32 * scale, actor.y as f32 * scale);
        for p in self.trail.iter().step_by(4) {
            let pt = rect.min + vec2(p[0] as f32 * scale, (p[1] + 8.) as f32 * scale);
            painter.rect_filled(
                Rect::from_center_size(pt, vec2(scale, scale)),
                0,
                Color32::from_rgba_unmultiplied(70, 85, 53, 70),
            );
        }
        if self.tutorial.beat > 0 {
            fly(
                &painter,
                center,
                scale,
                self.sim.world.tick,
                actor.vx,
                !actor.grounded,
            );
        }
        if let Some(s) = self.stroke.as_ref() {
            let c = if s.tool.erase() {
                Color32::from_rgba_unmultiplied(245, 242, 225, 160)
            } else {
                let [r, g, b] = s.color.unwrap_or([120, 124, 94]);
                Color32::from_rgba_unmultiplied(r, g, b, 150)
            };
            let pts: Vec<Pos2> = s
                .points
                .iter()
                .map(|p| rect.min + vec2(p[0] as f32 * scale, p[1] as f32 * scale))
                .collect();
            for pair in pts.windows(2) {
                painter.line_segment(
                    [pair[0], pair[1]],
                    Stroke::new(s.radius as f32 * scale * 2., c),
                );
            }
            if let Some(p) = pts.first() {
                painter.circle_filled(*p, s.radius as f32 * scale, c);
            }
        }
        if self.sim.world.outcome != Outcome::Running {
            let won = self.sim.world.outcome == Outcome::Won;
            let boxr = Rect::from_center_size(area.center(), vec2(220., 70.));
            painter.rect_filled(boxr, 4, PAPER);
            painter.rect_stroke(boxr, 4, Stroke::new(1., LINE), StrokeKind::Inside);
            painter.text(
                boxr.center() - vec2(0., 12.),
                Align2::CENTER_CENTER,
                if won {
                    "Still alive. You did it."
                } else {
                    "Oh, fly."
                },
                FontId::proportional(20.),
                INK,
            );
            painter.text(
                boxr.center() + vec2(0., 22.),
                Align2::CENTER_CENTER,
                if won {
                    if self.sim.world.level + 1 < world_core::LEVELS.len() {
                        "N for next level · R to go again"
                    } else {
                        "All six done · R to go again"
                    }
                } else {
                    "R to try again"
                },
                FontId::proportional(12.),
                MUTED,
            );
        }
        if !first {
            if self.tutorial.active() {
                self.intro_text(ui, area, rect);
            }
            return;
        }
        if response.hovered() && self.tutorial.can_paint() {
            ctx.set_cursor_icon(CursorIcon::Crosshair);
        }
        let pointer = ctx.input(|i| i.pointer.clone());
        if !self.tutorial.active()
            && self.zoom > 1.
            && response.hovered()
            && pointer.button_down(PointerButton::Middle)
        {
            self.pan += pointer.delta();
            self.pan.x = self.pan.x.clamp(-rect.width() / 2., rect.width() / 2.);
            self.pan.y = self.pan.y.clamp(-rect.height() / 2., rect.height() / 2.);
        }
        let ready = !matches!(
            self.sim.phase,
            Phase::Loading | Phase::Restoring | Phase::Fault
        ) && self.sim.world.outcome == Outcome::Running
            && !self.saving
            && self.restore_name.is_none()
            && self.tutorial.can_paint()
            && (self.tool != Tool::Ink || self.color.is_some());
        // Consume the ordered events, including the press position. Sampling
        // only PointerState::interact_pos loses fast drags contained in one frame.
        for event in ctx.input(|i| i.events.clone()) {
            match event {
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    ..
                } if ready
                    && self.pending_strokes.len() < 32
                    && area.contains(pos)
                    && rect.contains(pos)
                    && ctx.layer_id_at(pos) == Some(ui.layer_id()) =>
                {
                    let finishing_intro = self.tutorial.active();
                    if self.tutorial.begin_paint() {
                        self.log(serde_json::json!({"event":if finishing_intro { "tutorial_complete" } else { "first_paint" },"level":self.sim.world.level+1,"physics_tick":self.sim.world.tick}));
                    }
                    self.edit_notice = None;
                    self.gesture += 1;
                    self.stroke_dirty = true;
                    self.spacing = 1.;
                    self.stroke = Some(PaintStroke {
                        tool: self.tool,
                        points: vec![],
                        radius: self.radius,
                        color: self.color,
                    });
                    self.append_stroke_point(pos, rect, scale, true);
                }
                Event::PointerMoved(pos) => self.append_stroke_point(pos, rect, scale, false),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    ..
                } => {
                    self.append_stroke_point(pos, rect, scale, true);
                    if let Some(s) = self.stroke.take() {
                        self.pending_strokes.push_back((self.gesture, s));
                        self.stroke_dirty = false;
                    }
                }
                _ => {}
            }
        }
        if response.hovered() && self.tutorial.can_paint() {
            if let Some(p) = pointer.hover_pos() {
                painter.circle_stroke(p, self.radius as f32 * scale, Stroke::new(1., INK));
            }
        }
        if self.tutorial.active() {
            self.intro_text(ui, area, rect);
        }
    }
    fn intro_text(&self, ui: &mut Ui, area: Rect, map: Rect) {
        if self.tutorial.awaiting_start() {
            let ready =
                self.sim.phase == Phase::Paused && self.restore_name.is_none() && !self.saving;
            let text = if ready {
                "Press Enter to Start"
            } else if self.sim.phase == Phase::Fault {
                "Fly needs a restart"
            } else {
                "Getting Fly ready..."
            };
            tutorial_view::lettering(
                ui,
                Rect::from_center_size(
                    area.center() - vec2(0., 95.),
                    vec2((area.width() - 60.).min(760.), 70.),
                ),
                text,
                36.,
                INK,
            );
        } else {
            tutorial_view::narration(ui, area, map, &self.tutorial);
        }
    }
    fn append_stroke_point(&mut self, pos: Pos2, rect: Rect, scale: f32, force: bool) {
        if let Some(s) = &mut self.stroke {
            let q = [
                ((pos.x - rect.min.x) / scale).clamp(0., 639.) as f64,
                ((pos.y - rect.min.y) / scale).clamp(0., 359.) as f64,
            ];
            let q = [(q[0] * 256.).round() / 256., (q[1] * 256.).round() / 256.];
            if force
                || s.points
                    .last()
                    .is_none_or(|p| (p[0] - q[0]).hypot(p[1] - q[1]) >= self.spacing)
            {
                if s.points.len() >= 2048 {
                    s.points = s.points.iter().step_by(2).copied().collect();
                    self.spacing *= 2.;
                }
                s.points.push(q);
                self.stroke_dirty = true;
            }
        }
    }
}
