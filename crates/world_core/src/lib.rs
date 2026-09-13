mod body;
mod flag;
mod levels;
mod sensory;
mod terrain;
mod tutorial;
mod vision;
pub use levels::{LEVELS, LevelSpec};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub use terrain::{Change, Edit, Stroke, Tool, capsule_cells};
pub use tutorial::tutorial_cue;
pub const WIDTH: usize = 640;
pub const HEIGHT: usize = 360;
pub const COLS: usize = 160;
pub const ROWS: usize = 90;
pub const DT: f64 = 0.01;
pub const SENSOR_PROFILE: &str = "eye-level-v2";
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Action {
    pub steer: f64,
    pub jump: bool,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Actor {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub grounded: bool,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Outcome {
    Running,
    Won,
    Failed,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct World {
    #[serde(default, skip_serializing_if = "levels::is_zero")]
    pub level: usize,
    #[serde(default, skip_serializing_if = "levels::is_false")]
    pub button_pressed: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub layout_hash: String,
    pub base: Vec<u8>,
    pub solid: Vec<u8>,
    pub paint: Vec<u8>,
    pub actor: Actor,
    /// Horizontal camera heading, retained while stationary.
    pub facing: i8,
    pub tick: u64,
    pub revision: u64,
    pub outcome: Outcome,
    pub protected: Vec<[f64; 4]>,
    pub hazards: Vec<[f64; 4]>,
    pub goal: [f64; 4],
    pub history: Vec<Edit>,
    pub future: Vec<Edit>,
    pub step_up: bool,
}
impl World {
    pub fn bridge() -> Self {
        let mut base = vec![0; COLS * ROWS];
        for y in 70..90 {
            for x in 0..160 {
                if !(48..72).contains(&x) {
                    base[y * COLS + x] = 1;
                }
            }
        }
        // Uneven walls and ceiling give the editor real surfaces to color.
        for y in 0..ROWS {
            let left = 4 + (y / 7) % 3;
            let right = 4 + ((y + 5) / 9) % 3;
            for x in 0..COLS {
                if x < left || x >= COLS - right {
                    base[y * COLS + x] = 1;
                }
            }
        }
        for x in 0..COLS {
            let ceiling = 3 + (x / 11) % 3;
            for y in 0..ceiling {
                base[y * COLS + x] = 1;
            }
        }
        Self {
            level: 0,
            button_pressed: false,
            layout_hash: String::new(),
            base,
            solid: vec![0; COLS * ROWS],
            paint: vec![0; WIDTH * HEIGHT * 4],
            actor: Actor {
                x: 64.,
                y: 272.,
                vx: 0.,
                vy: 0.,
                grounded: true,
            },
            facing: 1,
            tick: 0,
            revision: 0,
            outcome: Outcome::Running,
            protected: vec![
                [0., 0., 640., 4.],
                [0., 0., 4., 360.],
                [636., 0., 4., 360.],
                [552., 232., 40., 48.],
            ],
            hazards: vec![[192., 352., 96., 8.]],
            goal: [560., 240., 24., 40.],
            history: vec![],
            future: vec![],
            step_up: true,
        }
    }
    pub fn snapshot(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("finite world state")
    }
    pub fn restore(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 12 * 1024 * 1024 {
            return Err("Checkpoint is too large".into());
        }
        let w: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if w.level >= LEVELS.len()
            || w.base.len() != COLS * ROWS
            || w.solid.len() != COLS * ROWS
            || w.paint.len() != WIDTH * HEIGHT * 4
            || w.base.iter().chain(&w.solid).any(|v| *v > 1)
            || w.paint.chunks_exact(4).any(|p| p[3] != 0 && p[3] != 255)
            || ![w.actor.x, w.actor.y, w.actor.vx, w.actor.vy]
                .iter()
                .all(|v| v.is_finite())
            || w.history.len() > 64
            || w.future.len() > 64
            || !matches!(w.facing, -1 | 1)
        {
            return Err("Invalid world checkpoint".into());
        }
        let original = Self::level(w.level);
        if w.base != original.base
            || w.goal != original.goal
            || w.protected != original.protected
            || w.hazards != original.hazards
            || w.layout_hash != original.layout_hash
            || (w.button_pressed && w.level_spec().button.is_none())
        {
            return Err("Incompatible level checkpoint".into());
        }
        w.validate_ground(&w.solid)?;
        Ok(w)
    }
    pub fn state_hash(&self) -> String {
        format!("{:x}", Sha256::digest(self.snapshot()))
    }
    pub fn occupied(&self, x: i32, y: i32) -> bool {
        if !(0..COLS as i32).contains(&x) {
            return true;
        }
        if !(0..ROWS as i32).contains(&y) {
            return false;
        }
        let i = y as usize * COLS + x as usize;
        self.base[i] != 0
            || self.solid[i] != 0
            || self.mechanism_solid([(x * 4) as f64, (y * 4) as f64, 4., 4.])
    }
}
pub fn overlaps(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] < b[0] + b[2] && a[0] + a[2] > b[0] && a[1] < b[1] + b[3] && a[1] + a[3] > b[1]
}
