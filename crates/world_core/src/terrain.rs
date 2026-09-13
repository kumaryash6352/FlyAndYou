use crate::*;
use std::collections::BTreeSet;
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Tool {
    Solid,
    Ink,
    EraseSolid,
    EraseInk,
}
impl Tool {
    pub fn solid(self) -> bool {
        matches!(self, Self::Solid | Self::EraseSolid)
    }
    pub fn erase(self) -> bool {
        matches!(self, Self::EraseSolid | Self::EraseInk)
    }
}
#[derive(Clone, Debug)]
pub struct Stroke {
    pub tool: Tool,
    pub points: Vec<[f64; 2]>,
    pub radius: f64,
    pub color: Option<[u8; 3]>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Change {
    pub index: usize,
    pub before: u8,
    pub after: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Edit {
    pub solid: bool,
    pub based_on_revision: u64,
    pub changes: Vec<Change>,
    /// Surface color accompanying a ground edit, committed and undone together.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paint_changes: Vec<Change>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wall_changes: Vec<Change>,
}
impl Edit {
    fn len(&self) -> usize {
        self.changes.len() + self.paint_changes.len() + self.wall_changes.len()
    }
}
fn paint_pixel(layer: &[u8], changes: &mut Vec<Change>, pixel: usize, rgba: [u8; 4]) {
    for (channel, after) in rgba.into_iter().enumerate() {
        let index = pixel * 4 + channel;
        if layer[index] != after {
            changes.push(Change {
                index,
                before: layer[index],
                after,
            });
        }
    }
}
fn validate_changes(layer: &[u8], changes: &[Change], rgba: bool) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    let mut pixels = std::collections::BTreeMap::<usize, [u8; 4]>::new();
    for c in changes {
        if c.index >= layer.len() || !seen.insert(c.index) || layer[c.index] != c.before {
            return Err("Stale or invalid edit".into());
        }
        if rgba {
            let p = c.index / 4;
            let value = pixels
                .entry(p)
                .or_insert_with(|| layer[p * 4..p * 4 + 4].try_into().unwrap());
            value[c.index % 4] = c.after;
        }
    }
    if pixels.values().any(|p| p[3] != 0 && p[3] != 255) {
        return Err("Ink must be opaque or erased".into());
    }
    Ok(())
}
fn merge_changes(prior: &mut Vec<Change>, changes: Vec<Change>) {
    let mut merged: std::collections::BTreeMap<usize, Change> =
        prior.drain(..).map(|c| (c.index, c)).collect();
    for c in changes {
        merged
            .entry(c.index)
            .and_modify(|old| old.after = c.after)
            .or_insert(c);
    }
    *prior = merged
        .into_values()
        .filter(|c| c.before != c.after)
        .collect();
}
fn inverse_changes(changes: &[Change]) -> Vec<Change> {
    changes
        .iter()
        .map(|c| Change {
            index: c.index,
            before: c.after,
            after: c.before,
        })
        .collect()
}
pub fn capsule_cells(points: &[[f64; 2]], radius: f64, cell: usize) -> Result<Vec<usize>, String> {
    if points.is_empty()
        || points.len() > 2048
        || !radius.is_finite()
        || radius <= 0.
        || radius > 24.
        || ![1, 4].contains(&cell)
    {
        return Err("Invalid stroke size".into());
    }
    if points
        .iter()
        .flatten()
        .any(|v| !v.is_finite() || *v < -24. || *v > 664.)
    {
        return Err("Stroke is outside the canvas".into());
    }
    let q: Vec<[i128; 2]> = points
        .iter()
        .map(|p| [(p[0] * 256.).round() as i128, (p[1] * 256.).round() as i128])
        .collect();
    let r = (radius * 256.).round() as i128;
    let unit = (cell * 256) as i128;
    let mut out = BTreeSet::new();
    for k in 0..q.len().max(2) - 1 {
        let a = q[k.min(q.len() - 1)];
        let b = q[(k + 1).min(q.len() - 1)];
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let len = dx * dx + dy * dy;
        let x0 = ((a[0].min(b[0]) - r) / unit - 1).max(0);
        let x1 = ((a[0].max(b[0]) + r) / unit + 1).min((WIDTH / cell - 1) as i128);
        let y0 = ((a[1].min(b[1]) - r) / unit - 1).max(0);
        let y1 = ((a[1].max(b[1]) + r) / unit + 1).min((HEIGHT / cell - 1) as i128);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let px = x * unit + unit / 2 - a[0];
                let py = y * unit + unit / 2 - a[1];
                let dot = px * dx + py * dy;
                let hit = if len == 0 || dot <= 0 {
                    px * px + py * py <= r * r
                } else if dot >= len {
                    (px - dx).pow(2) + (py - dy).pow(2) <= r * r
                } else {
                    (px * dy - py * dx).pow(2) <= r * r * len
                };
                if hit {
                    out.insert(y as usize * (WIDTH / cell) + x as usize);
                }
            }
        }
    }
    Ok(out.into_iter().collect())
}
impl World {
    pub fn plan(&self, s: &Stroke) -> Result<Edit, String> {
        self.plan_brush(s, false)
    }
    fn plan_brush(&self, s: &Stroke, live: bool) -> Result<Edit, String> {
        if s.tool.solid() && s.radius < 4. {
            return Err("Solid brush radius must be at least 4 pixels".into());
        }
        let cells = capsule_cells(&s.points, s.radius, if s.tool.solid() { 4 } else { 1 })?;
        let touched: BTreeSet<usize> = cells.iter().copied().collect();
        let mut changes = vec![];
        let mut paint_changes = vec![];
        let mut wall_changes = vec![];
        for i in cells {
            if live {
                let size = if s.tool.solid() { 4 } else { 1 };
                let width = WIDTH / size;
                let rect = [
                    (i % width * size) as f64,
                    (i / width * size) as f64,
                    size as f64,
                    size as f64,
                ];
                if (s.tool.solid() && self.protected.iter().any(|r| overlaps(*r, rect)))
                    || (s.tool == Tool::Solid
                        && overlaps(rect, [self.actor.x - 10., self.actor.y - 12., 20., 24.]))
                {
                    continue;
                }
                // Deterministic dry-brush edges. The roughness is in the actual
                // editable pixels/cells, so both eyes and collisions agree.
                let edge = [i.wrapping_sub(1), i + 1, i.wrapping_sub(width), i + width]
                    .iter()
                    .any(|n| !touched.contains(n));
                if !s.tool.erase()
                    && edge
                    && (i.wrapping_mul(73) ^ (i / width).wrapping_mul(151)) % 5 < 2
                {
                    continue;
                }
            }
            if s.tool.solid() {
                if self.base[i] != 0 || !self.ground_allowed(i) {
                    continue;
                }
                let after = if s.tool.erase() { 0 } else { 1 };
                if self.solid[i] != after {
                    changes.push(Change {
                        index: i,
                        before: self.solid[i],
                        after,
                    });
                }
                // Color the exact accepted collision cells, including their edges.
                // Erasing ground removes its ink and reveals the independent wall.
                let color = if s.tool.erase() {
                    (self.solid[i] != 0).then_some([0; 4])
                } else {
                    s.color.map(|[r, g, b]| [r, g, b, 255])
                };
                if let Some(rgba) = color {
                    for y in (i / COLS * 4)..(i / COLS * 4 + 4) {
                        for x in (i % COLS * 4)..(i % COLS * 4 + 4) {
                            let pixel = y * WIDTH + x;
                            let mut rgba = rgba;
                            if live && rgba[3] == 255 {
                                let grain =
                                    ((pixel.wrapping_mul(73) ^ y.wrapping_mul(151)) % 13) as u8;
                                for c in &mut rgba[..3] {
                                    *c = c.saturating_sub(grain);
                                }
                            }
                            paint_pixel(&self.paint, &mut paint_changes, pixel, rgba);
                        }
                    }
                }
            } else {
                let rgba = if s.tool.erase() {
                    [0; 4]
                } else if let Some(color) = s.color {
                    let grain = if live {
                        ((i.wrapping_mul(73) ^ (i / WIDTH).wrapping_mul(151)) % 13) as u8
                    } else {
                        0
                    };
                    [
                        color[0].saturating_sub(grain),
                        color[1].saturating_sub(grain),
                        color[2].saturating_sub(grain),
                        255,
                    ]
                } else {
                    continue;
                };
                if s.tool.erase() || self.paintable(i % WIDTH, i / WIDTH) {
                    paint_pixel(&self.paint, &mut changes, i, rgba);
                }
                if s.tool.erase() || !self.paintable(i % WIDTH, i / WIDTH) {
                    paint_pixel(&self.wall_paint, &mut wall_changes, i, rgba);
                }
            }
        }
        let e = Edit {
            solid: s.tool.solid(),
            based_on_revision: self.revision,
            changes,
            paint_changes,
            wall_changes,
        };
        self.validate_edit(&e)?;
        Ok(e)
    }
    pub fn validate_edit(&self, e: &Edit) -> Result<(), String> {
        if e.based_on_revision != self.revision {
            return Err("The world changed during this stroke".into());
        }
        let layer = if e.solid { &self.solid } else { &self.paint };
        validate_changes(layer, &e.changes, !e.solid)?;
        if !e.solid && !e.paint_changes.is_empty() {
            return Err("Unexpected compound ink edit".into());
        }
        validate_changes(&self.paint, &e.paint_changes, true)?;
        validate_changes(&self.wall_paint, &e.wall_changes, true)?;
        if e.solid {
            for c in &e.changes {
                let rect = [
                    ((c.index % COLS) * 4) as f64,
                    ((c.index / COLS) * 4) as f64,
                    4.,
                    4.,
                ];
                if self.protected.iter().any(|r| overlaps(*r, rect)) {
                    return Err("Keep the border and the little flag clear".into());
                }
                if c.after > 1 || self.base[c.index] != 0 {
                    return Err("Original ground cannot be changed".into());
                }
                if c.after == 1
                    && overlaps(rect, [self.actor.x - 10., self.actor.y - 12., 20., 24.])
                {
                    return Err("Give the fly a little room".into());
                }
            }
        }
        if e.solid {
            let mut next = self.solid.clone();
            for c in &e.changes {
                next[c.index] = c.after;
            }
            self.validate_ground(&next)?;
        }
        Ok(())
    }
    pub fn apply(&mut self, e: &Edit) -> Result<(), String> {
        self.validate_edit(e)?;
        if e.len() == 0 {
            return Ok(());
        }
        let layer = if e.solid {
            &mut self.solid
        } else {
            &mut self.paint
        };
        for c in &e.changes {
            layer[c.index] = c.after;
        }
        for c in &e.paint_changes {
            self.paint[c.index] = c.after;
        }
        for c in &e.wall_changes {
            self.wall_paint[c.index] = c.after;
        }
        self.revision += 1;
        Ok(())
    }
    pub fn edit(&mut self, s: &Stroke) -> Result<usize, String> {
        let e = self.plan(s)?;
        let n = e.len();
        self.apply(&e)?;
        if n > 0 {
            self.history.push(e);
            if self.history.len() > 64 {
                self.history.remove(0);
            }
            self.future.clear();
        }
        Ok(n)
    }
    pub fn edit_live(&mut self, s: &Stroke, continuation: bool) -> Result<usize, String> {
        let e = self.plan_brush(s, true)?;
        let n = e.len();
        self.apply(&e)?;
        if n > 0 {
            if continuation && self.history.last().is_some_and(|v| v.solid == e.solid) {
                let prior = self.history.last_mut().unwrap();
                merge_changes(&mut prior.changes, e.changes);
                merge_changes(&mut prior.paint_changes, e.paint_changes);
                merge_changes(&mut prior.wall_changes, e.wall_changes);
            } else {
                self.history.push(e);
                if self.history.len() > 64 {
                    self.history.remove(0);
                }
            }
            self.future.clear();
        }
        Ok(n)
    }
    pub fn undo(&mut self) -> Result<(), String> {
        let e = self.history.last().ok_or("Nothing to undo")?.clone();
        let inverse = Edit {
            solid: e.solid,
            based_on_revision: self.revision,
            changes: inverse_changes(&e.changes),
            paint_changes: inverse_changes(&e.paint_changes),
            wall_changes: inverse_changes(&e.wall_changes),
        };
        self.apply(&inverse)?;
        self.history.pop();
        self.future.push(e);
        Ok(())
    }
    pub fn redo(&mut self) -> Result<(), String> {
        let mut e = self.future.last().ok_or("Nothing to redo")?.clone();
        e.based_on_revision = self.revision;
        self.apply(&e)?;
        self.future.pop();
        self.history.push(e);
        Ok(())
    }
}
