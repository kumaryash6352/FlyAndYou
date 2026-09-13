//! Baked campaign geometry. Coordinates are pixels on the four-pixel grid.
use crate::*;
pub type Rect = [f64; 4];
#[derive(Serialize)]
pub struct LevelSpec {
    pub name: &'static str,
    /// Designer notes retained in existing checkpoint layout identities, not game hints.
    pub rule: &'static str,
    pub blocks: &'static [Rect],
    pub ground_zones: &'static [Rect],
    /// The first two Ground zones share one patch. Other zones are independent plugs.
    pub shared_ground: bool,
    pub plugs: &'static [Rect],
    pub hazards: &'static [Rect],
    pub start: [f64; 2],
    pub goal: Rect,
    pub button: Option<Rect>,
    pub gate: Option<Rect>,
    pub swatter: Option<Rect>,
}
pub static LEVELS: [LevelSpec; 6] = [
    LevelSpec {
        name: "First Flight",
        rule: "Help Fly reach the Goal.",
        blocks: &[],
        ground_zones: &[],
        shared_ground: false,
        plugs: &[],
        hazards: &[],
        start: [64., 272.],
        goal: [560., 240., 24., 40.],
        button: None,
        gate: None,
        swatter: None,
    },
    LevelSpec {
        name: "Borrowed Ground",
        rule: "One patch, two gaps. Ground fits in the dashed boxes. Erase the old bridge completely to reuse it in the other box.",
        blocks: &[
            [20., 260., 116., 100.],
            [216., 260., 176., 100.],
            [472., 260., 148., 100.],
        ],
        ground_zones: &[[136., 260., 80., 32.], [392., 260., 80., 32.]],
        shared_ground: true,
        plugs: &[],
        hazards: &[[136., 324., 80., 36.], [392., 324., 80., 36.]],
        start: [64., 252.],
        goal: [568., 220., 24., 40.],
        button: None,
        gate: None,
        swatter: None,
    },
    LevelSpec {
        name: "Touch and Go",
        rule: "Fly must step on A to open gate A. It stays open. The Goal is behind Fly; the far edge is dangerous.",
        blocks: &[[20., 260., 540., 100.]],
        ground_zones: &[],
        shared_ground: false,
        plugs: &[],
        hazards: &[[560., 260., 60., 100.]],
        start: [288., 252.],
        goal: [64., 220., 24., 40.],
        button: Some([432., 256., 24., 4.]),
        gate: Some([140., 16., 16., 244.]),
        swatter: None,
    },
    LevelSpec {
        name: "Pull the Rug",
        rule: "The stitched floor is erasable Ground. A opens the lower gate. Fly can fall onto safe ground below.",
        blocks: &[
            [20., 128., 280., 28.],
            [372., 128., 228., 28.],
            [20., 300., 580., 60.],
        ],
        ground_zones: &[[300., 128., 72., 28.]],
        shared_ground: false,
        plugs: &[[300., 128., 72., 28.]],
        hazards: &[[600., 128., 20., 172.]],
        start: [64., 120.],
        goal: [64., 260., 24., 40.],
        button: Some([456., 124., 24., 4.]),
        gate: Some([140., 156., 16., 144.]),
        swatter: None,
    },
    LevelSpec {
        name: "Swat Team",
        rule: "The swatter warns, then strikes. Build a stopping wall in the box; erase it when the path is clear. Space pauses the swatter too.",
        blocks: &[[20., 260., 600., 100.], [20., 122., 600., 28.]],
        ground_zones: &[[268., 172., 40., 88.]],
        shared_ground: false,
        plugs: &[],
        hazards: &[],
        start: [80., 252.],
        goal: [568., 220., 24., 40.],
        button: None,
        gate: None,
        swatter: Some([368., 150., 76., 110.]),
    },
    LevelSpec {
        name: "The Long Way Home",
        rule: "One patch for both gaps again. Open A, find a way down, then bring the Ground home. The stitched plug is separate from your shared patch.",
        blocks: &[
            [20., 128., 172., 28.],
            [280., 128., 252., 28.],
            [592., 128., 28., 28.],
            [20., 300., 292., 60.],
            [400., 300., 220., 60.],
            [612., 156., 8., 144.],
        ],
        ground_zones: &[
            [192., 128., 88., 28.],
            [312., 300., 88., 40.],
            [532., 128., 60., 28.],
        ],
        shared_ground: true,
        plugs: &[[532., 128., 60., 28.]],
        hazards: &[[192., 188., 88., 12.], [312., 340., 88., 20.]],
        start: [64., 120.],
        goal: [60., 260., 24., 40.],
        button: Some([400., 124., 24., 4.]),
        gate: Some([124., 156., 16., 144.]),
        swatter: None,
    },
];
fn fill(layer: &mut [u8], rect: Rect) {
    let [x, y, w, h] = rect;
    for cy in y as usize / 4..(y + h) as usize / 4 {
        for cx in x as usize / 4..(x + w) as usize / 4 {
            layer[cy * COLS + cx] = 1;
        }
    }
}
pub(crate) fn is_zero(v: &usize) -> bool {
    *v == 0
}
pub(crate) fn is_false(v: &bool) -> bool {
    !*v
}
impl World {
    pub fn level(index: usize) -> Self {
        assert!(index < LEVELS.len(), "unknown campaign level");
        let mut w = Self::bridge();
        if index == 0 {
            return w;
        }
        let spec = &LEVELS[index];
        w.level = index;
        w.layout_hash = format!("{:x}", Sha256::digest(serde_json::to_vec(spec).unwrap()));
        w.base.fill(0);
        // Continuous side walls; open bottom remains dangerous. The tutorial
        // retains its original rough geometry and exact sensory composition.
        for rect in [
            [0., 0., 20., 360.],
            [620., 0., 20., 360.],
            [0., 0., 640., 16.],
        ] {
            fill(&mut w.base, rect);
        }
        for &rect in spec.blocks {
            fill(&mut w.base, rect);
        }
        for &rect in spec.plugs {
            fill(&mut w.solid, rect);
        }
        w.actor.x = spec.start[0];
        w.actor.y = spec.start[1];
        w.goal = spec.goal;
        w.hazards = spec.hazards.to_vec();
        w.protected = vec![
            [0., 0., 640., 16.],
            [0., 0., 20., 360.],
            [620., 0., 20., 360.],
            [spec.goal[0] - 8., spec.goal[1] - 8., 40., 48.],
        ];
        if let Some(gate) = spec.gate {
            w.protected.push(gate);
        }
        if let Some(button) = spec.button {
            w.protected.push(button);
        }
        w
    }
    pub fn level_spec(&self) -> &'static LevelSpec {
        &LEVELS[self.level]
    }
    pub fn ground_zone(&self, index: usize) -> Option<usize> {
        let x = (index % COLS * 4) as f64;
        let y = (index / COLS * 4) as f64;
        self.level_spec()
            .ground_zones
            .iter()
            .position(|r| x >= r[0] && y >= r[1] && x + 4. <= r[0] + r[2] && y + 4. <= r[1] + r[3])
    }
    pub fn ground_allowed(&self, index: usize) -> bool {
        self.level == 0 || self.ground_zone(index).is_some()
    }
    pub fn shared_patch_zone(&self) -> Option<usize> {
        if !self.level_spec().shared_ground {
            return None;
        }
        self.solid
            .iter()
            .enumerate()
            .filter(|(_, v)| **v != 0)
            .filter_map(|(i, _)| self.ground_zone(i))
            .find(|&zone| zone < 2)
    }
    pub(crate) fn validate_ground(&self, solid: &[u8]) -> Result<(), String> {
        if self.level == 0 {
            return Ok(());
        }
        let mut used = [false; 2];
        for (i, &v) in solid.iter().enumerate().filter(|(_, v)| **v != 0) {
            let Some(zone) = self.ground_zone(i) else {
                return Err("Ground belongs inside the dashed boxes".into());
            };
            if v > 1 || self.base[i] != 0 {
                return Err("Invalid Ground cell".into());
            }
            if self.level_spec().shared_ground && zone < 2 {
                used[zone] = true;
            }
        }
        if used[0] && used[1] {
            return Err("Shared Ground is already in use".into());
        }
        Ok(())
    }
    /// Safe 2.5 s, warning 0.6 s, strike 0.9 s. Only physics ticks advance it.
    pub fn swatter_phase(&self) -> u8 {
        if self.level_spec().swatter.is_none() {
            return 0;
        }
        match self.tick % 400 {
            0..250 => 0,
            250..310 => 1,
            _ => 2,
        }
    }
    pub fn swatter_rect(&self) -> Option<Rect> {
        self.level_spec().swatter.map(|mut r| {
            if self.swatter_phase() != 2 {
                r[3] = 8.;
            }
            r
        })
    }
    pub(crate) fn mechanism_solid(&self, rect: Rect) -> bool {
        let spec = self.level_spec();
        spec.button.is_some_and(|r| overlaps(r, rect))
            || (!self.button_pressed && spec.gate.is_some_and(|r| overlaps(r, rect)))
    }
    pub(crate) fn hazard_at(&self, rect: Rect) -> bool {
        self.hazards.iter().any(|r| overlaps(*r, rect))
            || (self.swatter_phase() == 2 && self.swatter_rect().is_some_and(|r| overlaps(r, rect)))
    }
    pub(crate) fn ray_surface(&self, x: i32, y: i32) -> bool {
        let rect = [(x * 4) as f64, (y * 4) as f64, 4., 4.];
        self.occupied(x, y)
            || self.hazard_at(rect)
            || self.swatter_rect().is_some_and(|r| overlaps(r, rect))
    }
}
