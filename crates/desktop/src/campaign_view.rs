//! Hand-drawn editor annotations. These never enter the submitted eye image.
use crate::tutorial_view;
use bevy_egui::egui::*;
use world_core::World;

pub fn draw(painter: &Painter, map: Rect, scale: f32, w: &World) {
    let spec = w.level_spec();
    let at = |x: f64, y: f64| map.min + vec2(x as f32 * scale, y as f32 * scale);
    let color = Color32::from_rgb(96, 101, 86);
    let shared_patch = w.shared_patch_zone();
    for (index, r) in spec.ground_zones.iter().enumerate() {
        let shared = spec.shared_ground && index < 2;
        let blocked = shared && shared_patch.is_some_and(|used| used != index);
        let outline = if blocked {
            Color32::from_rgb(156, 112, 90)
        } else {
            color
        };
        let a = at(r[0], r[1]);
        let b = at(r[0] + r[2], r[1] + r[3]);
        for (p, q) in [
            (a, pos2(b.x, a.y)),
            (pos2(b.x, a.y), b),
            (b, pos2(a.x, b.y)),
            (pos2(a.x, b.y), a),
        ] {
            let delta = q - p;
            let len = delta.length();
            let direction = delta / len;
            let mut t = 0.;
            while t < len {
                painter.line_segment(
                    [p + direction * t, p + direction * (t + 4. * scale).min(len)],
                    Stroke::new(scale * 0.7, outline),
                );
                t += 8. * scale;
            }
        }
        let plug = spec.plugs.iter().any(|p| p == r);
        let text = if plug {
            "erasable"
        } else if shared {
            if blocked {
                "Ground\nin use elsewhere"
            } else {
                "Ground\none at a time"
            }
        } else {
            "Ground"
        };
        let caption_x = r[0] + r[2] / 2.;
        let caption_y = if shared && r[1] + r[3] > 320. {
            r[1] - 26.
        } else {
            r[1] + r[3] + if shared { 16. } else { 8. }
        };
        let on_terrain = w.occupied((caption_x / 4.) as i32, (caption_y / 4.) as i32)
            || w.hazards
                .iter()
                .any(|r| world_core::overlaps(*r, [caption_x, caption_y, 1., 1.]));
        label(
            painter,
            at(caption_x, caption_y),
            text,
            11. * scale,
            if on_terrain {
                Color32::from_rgb(235, 234, 218)
            } else {
                color
            },
        );
        if plug {
            for x in (r[0] as i32 + 6..(r[0] + r[2]) as i32).step_by(12) {
                if w.solid[(r[1] as usize / 4) * world_core::COLS + x as usize / 4] != 0 {
                    painter.line_segment(
                        [at(x as f64 - 2., r[1] + 4.), at(x as f64 + 2., r[1] + 10.)],
                        Stroke::new(scale, color),
                    );
                }
            }
        }
    }
    if let Some(r) = spec.button {
        label(
            painter,
            at(r[0] + r[2] / 2., r[1] - 14.),
            if w.button_pressed { "A pressed" } else { "A" },
            14. * scale,
            color,
        );
    }
    if let Some(r) = spec.gate {
        if !w.button_pressed {
            label(
                painter,
                at(r[0] + r[2] / 2., r[1] + r[3] * 0.65),
                "A",
                14. * scale,
                Color32::from_rgb(232, 232, 212),
            );
        }
    }
    if let Some(r) = spec.swatter {
        let text = match w.swatter_phase() {
            0 => "",
            1 => "!!",
            _ => "SWAT",
        };
        label(
            painter,
            at(r[0] + r[2] / 2., r[1] - 16.),
            text,
            15. * scale,
            Color32::from_rgb(235, 234, 218),
        );
    }
}

fn label(painter: &Painter, at: Pos2, text: &str, size: f32, color: Color32) {
    painter.text(
        at,
        Align2::CENTER_CENTER,
        text,
        tutorial_view::font(painter.ctx(), size),
        color,
    );
}

/// A rejected edit is labeled where it happened, beside the affected object.
pub fn edit_feedback(painter: &Painter, map: Rect, scale: f32, at: [f64; 2], message: &str) {
    let font = tutorial_view::font(painter.ctx(), 12. * scale);
    let galley = painter.layout(
        message.to_owned(),
        font,
        Color32::from_rgb(93, 63, 48),
        170. * scale,
    );
    let size = galley.size() + vec2(10., 6.);
    let point = map.min + vec2(at[0] as f32 * scale, at[1] as f32 * scale);
    let center = pos2(
        point
            .x
            .clamp(map.left() + size.x / 2., map.right() - size.x / 2.),
        (point.y - 22. * scale - size.y / 2.)
            .clamp(map.top() + size.y / 2., map.bottom() - size.y / 2.),
    );
    let area = Rect::from_center_size(center, size);
    painter.rect_filled(area, 2., tutorial_view::PAPER);
    painter.galley(
        area.min + vec2(5., 3.),
        galley,
        Color32::from_rgb(93, 63, 48),
    );
}
