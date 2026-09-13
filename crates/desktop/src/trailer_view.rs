use crate::{
    trailer::{Beat, Trailer},
    tutorial::smooth,
    tutorial_view::{self, PAPER, lettering},
};
use bevy_egui::egui::*;

const INK: Color32 = Color32::from_rgb(42, 45, 38);

pub fn loading(ctx: &Context, error: &str) {
    let mut viewport = Ui::new(
        ctx.clone(),
        Id::new("trailer-loading"),
        UiBuilder::new()
            .layer_id(LayerId::background())
            .max_rect(ctx.content_rect()),
    );
    CentralPanel::default()
        .frame(Frame::NONE)
        .show(&mut viewport, |ui| {
            let all = ui.max_rect();
            ui.painter().rect_filled(all, 0, PAPER);
            let rect = Rect::from_center_size(all.center(), vec2(all.width() - 80., 210.));
            lettering(ui, rect, "Waking Fly…", 48., INK);
            lettering(
                ui,
                rect.translate(vec2(0., 85.)),
                if error.is_empty() {
                    "If macOS asks, allow access to Documents."
                } else {
                    error
                },
                25.,
                INK,
            );
            small(
                ui,
                all,
                "Check for a permission dialog behind this window. The take waits for Enter.",
                40.,
            );
        });
}

pub fn draw(t: &mut Trailer, ctx: &Context) {
    t.app.textures(ctx);
    if ctx.current_pass_index() == 0 {
        ctx.input(|i| {
            t.app.focused = i.focused;
            if i.key_pressed(Key::Enter) {
                t.start();
            }
            if i.key_pressed(Key::Space) && t.started {
                t.paused = !t.paused;
            }
            if i.key_pressed(Key::R) && !i.modifiers.command {
                t.restart();
            }
        });
    }
    ctx.set_cursor_icon(if t.started && t.error.is_empty() {
        CursorIcon::None
    } else {
        CursorIcon::Default
    });
    let mut viewport = Ui::new(
        ctx.clone(),
        Id::new("trailer-root"),
        UiBuilder::new()
            .layer_id(LayerId::background())
            .max_rect(ctx.content_rect()),
    );
    CentralPanel::default()
        .frame(Frame::NONE)
        .show(&mut viewport, |ui| {
            let all = ui.max_rect();
            ui.painter().rect_filled(all, 0, PAPER);
            if t.beat == Beat::End && t.started {
                end_card(ui, all, t.elapsed);
            } else {
                let reveal = if !t.started || t.beat == Beat::Opening {
                    0.
                } else if t.beat == Beat::Premise {
                    smooth(t.elapsed as f32 / 1.1)
                } else {
                    1.
                };
                let rail_width = (all.width() * 0.215).clamp(220., 350.) * reveal;
                let game =
                    Rect::from_min_max(all.min, pos2(all.right() - rail_width, all.bottom()));
                let map = map_rect(t, game);
                let painter = ui.painter().with_clip_rect(game);
                if let Some(texture) = &t.app.world_texture {
                    painter.image(
                        texture.id(),
                        map,
                        Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                        Color32::WHITE,
                    );
                }
                let scale = map.width() / 640.;
                let a = &t.app.sim.world.actor;
                crate::campaign_view::draw(&painter, map, scale, &t.app.sim.world);
                let center = map.min + vec2(a.x as f32 * scale, a.y as f32 * scale);
                // Live shots use physics ticks. Only the held introductory portrait idles.
                let tick = if matches!(t.beat, Beat::Opening | Beat::Premise) {
                    (t.total * 100.) as u64
                } else {
                    t.app.sim.world.tick
                };
                crate::display::fly(&painter, center, scale, tick, a.vx, !a.grounded);
                if let Some((at, stroke)) = &t.cursor {
                    let p = map.min + vec2(at[0] as f32 * scale, at[1] as f32 * scale);
                    let color = if stroke.tool.erase() {
                        Color32::from_rgb(225, 221, 204)
                    } else if stroke.tool.solid() {
                        Color32::from_rgb(104, 114, 84)
                    } else {
                        Color32::from_rgb(stroke.color[0], stroke.color[1], stroke.color[2])
                    };
                    painter.circle_stroke(p, stroke.radius as f32 * scale, Stroke::new(1.4, color));
                    painter.line_segment(
                        [p + vec2(6., -6.), p + vec2(24., -30.)],
                        Stroke::new(5., INK),
                    );
                    painter.line_segment(
                        [p + vec2(5., -5.), p + vec2(12., -14.)],
                        Stroke::new(4., color),
                    );
                }
                if t.started {
                    caption(t, ui, game, map);
                    if reveal > 0.02 {
                        rail(
                            t,
                            ui,
                            Rect::from_min_max(pos2(game.right(), all.top()), all.max),
                        );
                    }
                }
            }
            if !t.started {
                let rect = Rect::from_center_size(all.center(), vec2(700., 180.));
                ui.painter().rect_filled(rect, 0, PAPER);
                lettering(ui, rect, "fly & you", 64., INK);
                lettering(
                    ui,
                    rect.translate(vec2(0., 92.)),
                    if t.ready() {
                        "Press Enter to roll"
                    } else {
                        "Waking Fly…"
                    },
                    26.,
                    INK,
                );
            small(
                ui,
                all,
                if t.ready() { "Enter starts a ~60 second take    ·    Space pauses    ·    R resets" }
                else { "If macOS asks, allow Documents access. Check for a dialog behind this window." },
                    26.,
                );
            }
            let error = if !t.error.is_empty() {
                t.error.as_str()
            } else {
                t.app.sim.error.as_str()
            };
            if !error.is_empty() {
                let r = Rect::from_center_size(all.center(), vec2(760., 180.));
                ui.painter().rect_filled(r, 0, PAPER);
                lettering(
                    ui,
                    r.shrink(20.),
                    error,
                    27.,
                    Color32::from_rgb(155, 55, 40),
                );
            } else if t.paused {
                small(ui, all, "Paused · Space to continue", 35.);
            }
        });
}

fn map_rect(t: &Trailer, area: Rect) -> Rect {
    let (center, zoom) = match t.beat {
        Beat::Opening => {
            let time = t.elapsed;
            if time < 4.6 {
                (vec2(64., 225.), 2.35)
            } else if time < 5.8 {
                let k = smooth(((time - 4.6) / 1.2) as f32);
                (vec2(64., 225.) + vec2(508., -10.) * k, 2.35)
            } else if time < 9.3 {
                (vec2(572., 215.), 2.35)
            } else {
                let k = smooth(((time - 9.3) / 1.2) as f32);
                (
                    vec2(572., 215.) + (vec2(320., 180.) - vec2(572., 215.)) * k,
                    2.35 - 1.35 * k,
                )
            }
        }
        Beat::Payoff => {
            let k = smooth(t.elapsed as f32 / 1.4);
            (vec2(320., 180.) + vec2(188., 47.) * k, 1. + 0.7 * k)
        }
        Beat::Red => (vec2(345., 225.), 1.6),
        Beat::Rug => (vec2(362., 206.), 1.22),
        Beat::Wall => (vec2(398., 212.), 1.6),
        _ => (vec2(320., 180.), 1.),
    };
    let scale = (area.width() / 640.).min(area.height() / 360.) * zoom;
    Rect::from_min_size(area.center() - center * scale, vec2(640., 360.) * scale)
}

fn caption(t: &Trailer, ui: &mut Ui, area: Rect, map: Rect) {
    let text = match t.beat {
        Beat::Opening => match t.elapsed {
            x if (0.7..3.6).contains(&x) => "This is Fly",
            x if (6.1..8.6).contains(&x) => "That is the goal",
            x if x >= 10.8 => "Fly does not know that",
            _ => "",
        },
        Beat::Premise if t.elapsed < 5.9 => "You cannot control Fly",
        Beat::Construction if t.elapsed < 5.9 => "You can change his world.",
        Beat::Persuasion if t.elapsed < 3. => "A little yellow…",
        Beat::Persuasion if t.saw_approach && t.elapsed < 5.5 => "…gets his attention.",
        Beat::Payoff if t.elapsed > 0.6 => "You're his friend.",
        _ => "",
    };
    let width = (area.width() - 60.).min(850.);
    let rect = Rect::from_min_size(
        pos2(area.center().x - width / 2., area.top() + 48.),
        vec2(width, 120.),
    );
    let bounds = lettering(ui, rect, text, (area.width() / 25.).clamp(28., 44.), INK);
    if t.beat == Beat::Opening && !text.is_empty() && t.elapsed < 9. {
        let target = if t.elapsed < 4. {
            vec2(64., 260.)
        } else {
            vec2(572., 235.)
        };
        tutorial_view::arrow(
            &ui.painter().with_clip_rect(area),
            bounds.center_bottom() + vec2(25., 8.),
            map.min + target * (map.width() / 640.) - vec2(0., 12.),
        );
    }
}

fn rail(t: &mut Trailer, ui: &mut Ui, rect: Rect) {
    let brain = Rect::from_min_max(
        rect.min,
        pos2(rect.right(), rect.top() + rect.height() * 0.59),
    );
    t.app.brain_view.paint(
        ui,
        brain,
        t.app.telemetry.ticks,
        &t.app.telemetry.anatomy_activity,
    );
    let eye = Rect::from_min_max(pos2(rect.left(), brain.bottom() + 1.), rect.max);
    ui.painter()
        .rect_filled(eye, 0, Color32::from_rgb(21, 24, 25));
    if let Some(texture) = &t.app.vision_texture {
        let size = vec2(128., 96.) * (eye.width() / 128.).min((eye.height() - 42.) / 96.);
        ui.painter().image(
            texture.id(),
            Rect::from_center_size(eye.center() + vec2(0., 16.), size),
            Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
            Color32::WHITE,
        );
    }
    lettering(
        ui,
        Rect::from_min_size(eye.min + vec2(4., 6.), vec2(eye.width() - 8., 34.)),
        "FlyCam™",
        24.,
        Color32::from_rgb(224, 225, 211),
    );
}

fn end_card(ui: &mut Ui, area: Rect, seconds: f64) {
    let step = (seconds * 3.).floor() as f32;
    let shuffle = vec2((step * 1.7).sin() * 1.6, (step * 2.3).cos() * 1.2);
    let width = area.width() * 0.9;
    let size = (area.width() / 10.).clamp(72., 134.);
    let title = Rect::from_min_size(
        pos2(area.center().x - width / 2., area.center().y - 110.) + shuffle,
        vec2(width, 170.),
    );
    lettering(ui, title, "fly & you", size, INK);
    let tag = Rect::from_min_size(
        pos2(area.center().x - width / 2., area.center().y + 55.) - shuffle,
        vec2(width, 80.),
    );
    lettering(
        ui,
        tag,
        "a half-player platformer",
        (area.width() / 38.).clamp(24., 37.),
        INK,
    );
    crate::display::fly(
        ui.painter(),
        area.center() + vec2(0., 160.),
        2.,
        (seconds * 100.) as u64,
        0.,
        false,
    );
}

fn small(ui: &mut Ui, area: Rect, text: &str, bottom: f32) {
    ui.painter().text(
        pos2(area.center().x, area.bottom() - bottom),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(16.),
        INK,
    );
}
