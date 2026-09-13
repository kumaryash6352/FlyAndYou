use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
pub const PROTOCOL_VERSION: u32 = 2;
pub const NEURAL_STEPS: u32 = 2;
pub const PHYSICS_TICKS: u64 = 4;
pub const DECISION_SECONDS: f64 = 0.04;
pub const MAX_MESSAGE: usize = 1_048_576;
pub fn hash(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
pub fn valid_hex(s: &str, length: usize) -> bool {
    s.len() == length
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub kind: String,
    pub version: u32,
    pub epoch: String,
    pub step_id: u64,
    pub physics_tick: u64,
    pub world_revision: u64,
    pub profile_sha256: String,
    pub rgb_sha256: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub neural_steps: u32,
    pub frame_b64: String,
}
impl Step {
    pub fn validate(&self) -> Result<Vec<u8>, String> {
        if self.kind != "step"
            || self.version != PROTOCOL_VERSION
            || !valid_hex(&self.epoch, 32)
            || !valid_hex(&self.profile_sha256, 64)
            || !valid_hex(&self.rgb_sha256, 64)
            || self.width != 128
            || self.height != 96
            || self.format != "rgb8"
            || self.neural_steps != NEURAL_STEPS
            || self.step_id.checked_mul(PHYSICS_TICKS) != Some(self.physics_tick)
        {
            return Err("Invalid 25 Hz observation identity or dimensions".into());
        }
        let rgb = STANDARD
            .decode(&self.frame_b64)
            .map_err(|e| e.to_string())?;
        if rgb.len() != 128 * 96 * 3 || hash(&rgb) != self.rgb_sha256 {
            return Err("Observation pixels do not match image hash".into());
        }
        Ok(rgb)
    }
    pub fn new(
        epoch: String,
        step_id: u64,
        physics_tick: u64,
        world_revision: u64,
        profile_sha256: String,
        rgb: &[u8],
    ) -> Self {
        Self {
            kind: "step".into(),
            version: PROTOCOL_VERSION,
            epoch,
            step_id,
            physics_tick,
            world_revision,
            profile_sha256,
            rgb_sha256: hash(rgb),
            width: 128,
            height: 96,
            format: "rgb8".into(),
            neural_steps: NEURAL_STEPS,
            frame_b64: STANDARD.encode(rgb),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub steer: f64,
    pub jump: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub kind: String,
    pub version: u32,
    pub epoch: String,
    pub step_id: u64,
    pub physics_tick: u64,
    pub world_revision: u64,
    pub profile_sha256: String,
    pub rgb_sha256: String,
    pub neural_steps_done: u32,
    pub action: Action,
}
impl Reply {
    pub fn for_request(r: &Step, steer: f64) -> Self {
        Self {
            kind: "action".into(),
            version: r.version,
            epoch: r.epoch.clone(),
            step_id: r.step_id,
            physics_tick: r.physics_tick,
            world_revision: r.world_revision,
            profile_sha256: r.profile_sha256.clone(),
            rgb_sha256: r.rgb_sha256.clone(),
            neural_steps_done: NEURAL_STEPS,
            action: Action { steer, jump: false },
        }
    }
    pub fn validate(&self, r: &Step) -> Result<(), String> {
        if self.kind != "action"
            || self.version != PROTOCOL_VERSION
            || self.version != r.version
            || self.epoch != r.epoch
            || self.step_id != r.step_id
            || self.physics_tick != r.physics_tick
            || self.world_revision != r.world_revision
            || self.profile_sha256 != r.profile_sha256
            || self.rgb_sha256 != r.rgb_sha256
            || self.neural_steps_done != NEURAL_STEPS
            || self.neural_steps_done != r.neural_steps
            || self.action.jump
            || !self.action.steer.is_finite()
            || self.action.steer.abs() > 1.
        {
            Err("Controller reply does not match the observation".into())
        } else {
            Ok(())
        }
    }
}
pub fn write_frame<W: Write, T: Serialize>(w: &mut W, v: &T) -> Result<(), String> {
    let data = serde_json::to_vec(v).map_err(|e| e.to_string())?;
    if data.is_empty() || data.len() > MAX_MESSAGE {
        return Err("Frame exceeds size limit".into());
    }
    w.write_all(&(data.len() as u32).to_be_bytes())
        .and_then(|_| w.write_all(&data))
        .and_then(|_| w.flush())
        .map_err(|e| e.to_string())
}
pub fn read_frame<R: Read, T: DeserializeOwned>(r: &mut R) -> Result<T, String> {
    let mut header = [0; 4];
    r.read_exact(&mut header).map_err(|e| e.to_string())?;
    let n = u32::from_be_bytes(header) as usize;
    if n == 0 || n > MAX_MESSAGE {
        return Err("Invalid frame length".into());
    }
    let mut bytes = vec![0; n];
    r.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
