use crate::tutorial::{Tutorial, smooth};
use bevy_egui::egui::{epaint::TextShape, *};

pub const PAPER: Color32 = Color32::from_rgb(235, 234, 218);
const INK: Color32 = Color32::from_rgb(42, 45, 38);
const HAND: &str = "hand-drawn";

pub fn install_font(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    // Use the system's licensed font in place, without redistributing it.
    for path in [
        "/System/Library/Fonts/Supplemental/ChalkboardSE.ttc",
        "/System/Library/Fonts/Supplemental/Comic Sans MS.ttf",
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert(HAND.into(), FontData::from_owned(bytes).into());
            let mut family = vec![HAND.into()];
            family.extend(fonts.families[&FontFamily::Proportional].clone());
            fonts.families.insert(FontFamily::Name(HAND.into()), family);
            ctx.set_fonts(fonts);
            return;
        }
    }
}

fn font(ctx: &Context, size: f32) -> FontId {
    let family = ctx.fonts(|f| {
        if f.families().contains(&FontFamily::Name(HAND.into())) {
            FontFamily::Name(HAND.into())
        } else {
            FontFamily::Proportional
        }
    });
    FontId::new(size, family)
}

/// Stable irregular lettering: the text does not twitch as frames advance.
pub fn lettering(ui: &mut Ui, area: Rect, text: &str, size: f32, color: Color32) -> Rect {
    if text.is_empty() {
        return Rect::NOTHING;
    }
    let painter = ui.painter().with_clip_rect(area);
    let font = font(ui.ctx(), size);
    let space = size * 0.34;
    let mut lines = vec![Vec::new()];
    let mut widths = vec![0.];
    for (index, word) in text.split_whitespace().enumerate() {
        let tint = match word {
            "Red" => Color32::from_rgb(179, 57, 41),
            "Yellow" => Color32::from_rgb(141, 105, 4),
            _ => color,
        };
        let glyphs: Vec<_> = word
            .chars()
            .map(|c| painter.layout_no_wrap(c.to_string(), font.clone(), tint))
            .collect();
        let width: f32 = glyphs.iter().map(|g| g.size().x).sum();
        let last = widths.len() - 1;
        if !lines[last].is_empty() && widths[last] + space + width > area.width() - 12. {
            lines.push(Vec::new());
            widths.push(0.);
        }
        let last = widths.len() - 1;
        if !lines[last].is_empty() {
            widths[last] += space;
        }
        widths[last] += width;
        lines[last].push((index, glyphs, tint));
    }
    let height = lines.len() as f32 * size * 1.35;
    let bounds = Rect::from_center_size(
        pos2(area.center().x, area.top() + height * 0.5 + 3.),
        vec2(widths.iter().copied().fold(0., f32::max), height),
    );
    for (line, words) in lines.into_iter().enumerate() {
        let mut x = area.center().x - widths[line] * 0.5;
        let y = area.top() + 3. + line as f32 * size * 1.35;
        for (index, glyphs, tint) in words {
            for (letter, galley) in glyphs.into_iter().enumerate() {
                let seed = (index * 31 + letter * 17 + 11) as f32;
                let width = galley.size().x;
                let p = pos2(x + seed.sin() * 0.35, y + (seed * 1.7).sin() * 0.8);
                painter.add(TextShape::new(p, galley, tint).with_angle(seed.cos() * 0.025));
                x += width;
            }
            x += space;
        }
    }
    ui.interact(bounds, Id::new(("intro-caption", text)), Sense::hover())
        .widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, text));
    bounds
}

pub fn map_rect(area: Rect, intro: &Tutorial) -> Rect {
    let fit = (area.width() / 640.).min(area.height() / 360.);
    let fly = vec2(64., 272.);
    let goal = vec2(568., 260.);
    let full = vec2(320., 180.);
    let (center, zoom) = match intro.beat {
        0..=2 => (fly, 2.4),
        3 => (fly + (goal - fly) * smooth(intro.progress()), 2.4),
        4..=5 => (goal, 2.4),
        6 => {
            let t = smooth(intro.progress());
            (goal + (full - goal) * t, 2.4 + (1. - 2.4) * t)
        }
        _ => (full, 1.),
    };
    let scale = fit * zoom;
    Rect::from_min_size(area.center() - center * scale, vec2(640., 360.) * scale)
}

fn arrow(painter: &Painter, start: Pos2, end: Pos2) {
    let delta = end - start;
    let normal = vec2(-delta.y, delta.x).normalized();
    let points: Vec<_> = (0..=8)
        .map(|i| {
            let t = i as f32 / 8.;
            start
                + delta * t
                + normal * ((i as f32 * 2.1).sin() * 1.4 + (t * std::f32::consts::PI).sin() * 9.)
        })
        .collect();
    painter.add(Shape::line(points, Stroke::new(1.7, INK)));
    let back = end - delta.normalized() * 13.;
    painter.line_segment([back + normal * 5., end], Stroke::new(1.8, INK));
    painter.line_segment([end, back - normal * 6.], Stroke::new(1.5, INK));
}

pub fn narration(ui: &mut Ui, area: Rect, map: Rect, intro: &Tutorial) {
    let text = intro.caption();
    let width = (area.width() - 90.).min(760.);
    let size = if matches!(intro.beat, 9 | 10) {
        27.
    } else {
        32.
    };
    // Once the toolbox arrives, use the open space beneath it, above Fly.
    let top = if intro.show_tools() {
        area.top() + 265.
    } else if intro.beat >= 6 {
        map.top() + map.width() / 640. * 36.
    } else {
        area.top() + 82.
    };
    let caption = Rect::from_min_size(pos2(area.center().x - width * 0.5, top), vec2(width, 260.));
    let bounds = lettering(ui, caption, text, size, INK);
    if matches!(intro.beat, 2 | 5 | 13 | 14) {
        let scale = map.width() / 640.;
        let target = match intro.beat {
            2 => vec2(64., 264.),
            5 => vec2(572., 240.),
            _ => vec2(100., 240.),
        };
        let end = map.min + target * scale - vec2(0., 14.);
        arrow(
            &ui.painter().with_clip_rect(area),
            bounds.center_bottom() + vec2(35., 8.),
            end,
        );
    }
}
