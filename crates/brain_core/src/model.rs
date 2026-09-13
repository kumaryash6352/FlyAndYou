use crate::{backend::Compute, motor::MotorState, profile::Profile, rng::NumpyRng};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const SOURCES: &[(&str, &[u8])] = &[
    ("crates/brain_core/src/model.rs", include_bytes!("model.rs")),
    (
        "crates/brain_core/src/profile.rs",
        include_bytes!("profile.rs"),
    ),
    ("crates/brain_core/src/motor.rs", include_bytes!("motor.rs")),
    ("crates/brain_core/src/rng.rs", include_bytes!("rng.rs")),
    (
        "crates/brain_core/src/backend.rs",
        include_bytes!("backend.rs"),
    ),
    (
        "crates/brain_core/src/step.metal",
        include_bytes!("step.metal"),
    ),
    (
        "crates/brain_core/Cargo.toml",
        include_bytes!("../Cargo.toml"),
    ),
];
pub fn source_hashes() -> std::collections::BTreeMap<String, String> {
    SOURCES
        .iter()
        .map(|(n, b)| (n.to_string(), wire_types::hash(b)))
        .collect()
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub schema: u32,
    pub backend: String,
    pub profile_sha256: String,
    pub activity: String,
    pub previous: String,
    pub rng: NumpyRng,
    pub ticks: u64,
    pub connected: bool,
    pub motor: MotorState,
}
pub struct Brain {
    pub profile: Profile,
    pub profile_hash: String,
    pub activity: Vec<f32>,
    pub previous: Vec<f32>,
    pub ticks: u64,
    pub connected: bool,
    pub motor: MotorState,
    rng: NumpyRng,
    compute: Compute,
}
fn encode_f32(a: &[f32]) -> String {
    STANDARD.encode(a.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<_>>())
}
fn decode_f32(s: &str, n: usize) -> Result<Vec<f32>, String> {
    let b = STANDARD.decode(s).map_err(|e| e.to_string())?;
    if b.len() != n * 4 {
        return Err("Checkpoint dimensions mismatch".into());
    }
    let a: Vec<_> = b
        .chunks_exact(4)
        .map(|x| f32::from_le_bytes(x.try_into().unwrap()))
        .collect();
    if a.iter().any(|x| !x.is_finite() || !(0.0..=1.0).contains(x)) {
        return Err("Invalid checkpoint activity/current".into());
    }
    Ok(a)
}
fn mean(a: &[f32], indices: &[usize]) -> f32 {
    (indices.iter().map(|i| a[*i] as f64).sum::<f64>() / indices.len() as f64) as f32
}
impl Brain {
    pub fn load(root: &Path, cpu: bool) -> Result<Self, String> {
        Self::from_profile(Profile::load(root)?, cpu)
    }
    #[cfg(feature = "embedded-model")]
    pub fn embedded(cpu: bool) -> Result<Self, String> {
        Self::from_profile(Profile::embedded()?, cpu)
    }
    fn from_profile(profile: Profile, cpu: bool) -> Result<Self, String> {
        let compute = Compute::new(&profile, cpu)?;
        let identity = serde_json::json!({"prepared":profile.hash,"backend":compute.name(),"sources":source_hashes(),"candle":"0.11.0","protocol":wire_types::PROTOCOL_VERSION});
        let profile_hash =
            wire_types::hash(&serde_json::to_vec(&identity).map_err(|e| e.to_string())?);
        let activity = vec![0.; profile.manifest.nodes];
        let previous = vec![0.; profile.mapping.inputs.len()];
        let rng = profile.initial_rng.clone();
        Ok(Self {
            profile,
            profile_hash,
            activity,
            previous,
            rng,
            compute,
            ticks: 0,
            connected: true,
            motor: MotorState::default(),
        })
    }
    pub fn backend(&self) -> &str {
        self.compute.name()
    }
    pub fn evidence(&self) -> [f32; 2] {
        let m = &self.profile.mapping;
        let r = &self.profile.manifest.readout;
        [
            (mean(&self.activity, &m.approach_indices) - r.neutral_baseline[0]).max(0.) / r.scale,
            (mean(&self.activity, &m.avoid_indices) - r.neutral_baseline[1]).max(0.) / r.scale,
        ]
    }
    pub fn step_image(&mut self, rgb: &[u8]) -> Result<f64, String> {
        if rgb.len() != 128 * 96 * 3 {
            return Err("Expected 128 x 96 RGB8".into());
        }
        let m = &self.profile.mapping;
        let mut input = vec![0.; self.activity.len()];
        let mut previous = vec![0.; m.inputs.len()];
        for (i, ([x, y], yellow)) in m.samples.iter().zip(&m.yellow_mask).enumerate() {
            let offset = (y * 128 + x) * 3;
            let r = rgb[offset] as f32;
            let g = rgb[offset + 1] as f32;
            let b = rgb[offset + 2] as f32;
            let current = if *yellow {
                ((r.min(g) - b - 24.) / 160.).clamp(0., 1.)
            } else {
                ((r - g.max(b) - 48.) / 160.).clamp(0., 1.)
            };
            previous[i] = current;
            if self.connected {
                input[m.inputs[i]] = current;
            }
        }
        let mut rng = self.rng.clone();
        let noise: Vec<Vec<f32>> = (0..wire_types::NEURAL_STEPS)
            .map(|_| {
                (0..self.activity.len())
                    .map(|_| rng.normal_f32(self.profile.manifest.dynamics.noise))
                    .collect()
            })
            .collect();
        let next = self.compute.run(&self.profile, &input, &noise)?;
        if next
            .iter()
            .any(|x| !x.is_finite() || !(0.0..=1.0).contains(x))
        {
            return Err("Non-finite/out-of-range neural activity".into());
        }
        self.activity = next;
        self.rng = rng;
        self.previous = previous;
        self.ticks = self
            .ticks
            .checked_add(wire_types::NEURAL_STEPS as u64)
            .ok_or("Neural clock overflow")?;
        let [approach, avoidance] = self.evidence();
        Ok(self.motor.step(
            approach as f64,
            avoidance as f64,
            &self.profile.manifest.motor,
        ))
    }
    pub fn inspect(&self) -> serde_json::Value {
        let [approach, avoidance] = self.evidence();
        let m = &self.profile.mapping;
        let take = |v: &[usize]| v.iter().map(|i| self.activity[*i]).collect::<Vec<_>>();
        serde_json::json!({"ticks":self.ticks,"left":mean(&self.activity,&m.left),"right":mean(&self.activity,&m.right),
            "input_mean":self.previous.iter().map(|x|*x as f64).sum::<f64>()/self.previous.len() as f64,
            "activity":take(&m.telemetry),"anatomy_activity":take(&self.profile.anatomy),"steer":self.motor.steer,
            "nodes":self.activity.len(),"edges":self.profile.values.len(),"connected":self.connected,
            "heading":self.motor.heading,"search_age":self.motor.search_age,"approach":approach,"avoidance":avoidance,
            "motor_mode":self.motor.mode,"turn_decisions_remaining":self.motor.remaining,
            "backend":self.backend(),"decisions_hz":25,"model":"leaky rate; engineered chromatic valence"})
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            schema: 1,
            backend: self.backend().into(),
            profile_sha256: self.profile_hash.clone(),
            activity: encode_f32(&self.activity),
            previous: encode_f32(&self.previous),
            rng: self.rng.clone(),
            ticks: self.ticks,
            connected: self.connected,
            motor: self.motor.clone(),
        }
    }
    pub fn restore(&mut self, s: &Snapshot) -> Result<(), String> {
        if s.schema != 1
            || s.backend != self.backend()
            || s.profile_sha256 != self.profile_hash
            || s.ticks % (wire_types::NEURAL_STEPS as u64) != 0
            || s.ticks > u64::MAX / 2
        {
            return Err("Incompatible brain checkpoint".into());
        }
        let a = decode_f32(&s.activity, self.activity.len())?;
        let p = decode_f32(&s.previous, self.previous.len())?;
        s.rng.validate()?;
        s.motor.validate(&self.profile.manifest.motor)?;
        // Validate the complete replacement before touching either CPU or GPU state.
        self.compute.restore(&a)?;
        self.activity = a;
        self.previous = p;
        self.rng = s.rng.clone();
        self.motor = s.motor.clone();
        self.ticks = s.ticks;
        self.connected = s.connected;
        Ok(())
    }
}
