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
    pub color: [u8; 3],
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
                if (self.protected.iter().any(|r| overlaps(*r, rect))
                    && (s.tool.solid() || !self.paintable(i % width, i / width)))
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
                if self.base[i] != 0 {
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
            } else {
                if !self.paintable(i % WIDTH, i / WIDTH) && !s.tool.erase() {
                    continue;
                }
                let rgba = if s.tool.erase() {
                    [0, 0, 0, 0]
                } else {
                    let grain = if live {
                        ((i.wrapping_mul(73) ^ (i / WIDTH).wrapping_mul(151)) % 13) as u8
                    } else {
                        0
                    };
                    [
                        s.color[0].saturating_sub(grain),
                        s.color[1].saturating_sub(grain),
                        s.color[2].saturating_sub(grain),
                        255,
                    ]
                };
                for (c, after) in rgba.into_iter().enumerate() {
                    let index = i * 4 + c;
                    if self.paint[index] != after {
                        changes.push(Change {
                            index,
                            before: self.paint[index],
                            after,
                        });
                    }
                }
            }
        }
        let e = Edit {
            solid: s.tool.solid(),
            based_on_revision: self.revision,
            changes,
        };
        self.validate_edit(&e)?;
        Ok(e)
    }
    pub fn validate_edit(&self, e: &Edit) -> Result<(), String> {
        if e.based_on_revision != self.revision {
            return Err("The world changed during this stroke".into());
        }
        let layer = if e.solid { &self.solid } else { &self.paint };
        let mut seen = BTreeSet::new();
        for c in &e.changes {
            if c.index >= layer.len() || !seen.insert(c.index) || layer[c.index] != c.before {
                return Err("Stale or invalid edit".into());
            }
            let (x, y, size) = if e.solid {
                ((c.index % COLS) * 4, (c.index / COLS) * 4, 4)
            } else {
                ((c.index / 4) % WIDTH, (c.index / 4) / WIDTH, 1)
            };
            let rect = [x as f64, y as f64, size as f64, size as f64];
            if self.protected.iter().any(|r| overlaps(*r, rect))
                && (e.solid || !self.paintable(x, y))
            {
                return Err("Keep the border and the little flag clear".into());
            }
            if e.solid {
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
        if !e.solid {
            let mut pixels = std::collections::BTreeMap::<usize, [u8; 4]>::new();
            for c in &e.changes {
                let p = c.index / 4;
                let rgba = pixels
                    .entry(p)
                    .or_insert_with(|| self.paint[p * 4..p * 4 + 4].try_into().unwrap());
                rgba[c.index % 4] = c.after;
            }
            if pixels.values().any(|p| p[3] != 0 && p[3] != 255) {
                return Err("Ink must be opaque or erased".into());
            }
        }
        Ok(())
    }
    pub fn apply(&mut self, e: &Edit) -> Result<(), String> {
        self.validate_edit(e)?;
        if e.changes.is_empty() {
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
        self.revision += 1;
        Ok(())
    }
    pub fn edit(&mut self, s: &Stroke) -> Result<usize, String> {
        let e = self.plan(s)?;
        let n = e.changes.len();
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
        let n = e.changes.len();
        self.apply(&e)?;
        if n > 0 {
            if continuation && self.history.last().is_some_and(|v| v.solid == e.solid) {
                let prior = self.history.last_mut().unwrap();
                let mut merged: std::collections::BTreeMap<usize, Change> = prior
                    .changes
                    .iter()
                    .cloned()
                    .map(|c| (c.index, c))
                    .collect();
                for c in e.changes {
                    merged
                        .entry(c.index)
                        .and_modify(|old| old.after = c.after)
                        .or_insert(c);
                }
                prior.changes = merged
                    .into_values()
                    .filter(|c| c.before != c.after)
                    .collect();
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
            changes: e
                .changes
                .iter()
                .map(|c| Change {
                    index: c.index,
                    before: c.after,
                    after: c.before,
                })
                .collect(),
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
