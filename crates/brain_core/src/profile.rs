use crate::{motor::MotorConfig, rng::NumpyRng};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
};
use wire_types::hash;

pub const BACKEND: &str = "candle-metal-csr-f32-v1";
pub const RNG: &str = "numpy-pcg64-normal-f64-v1";
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Dynamics {
    pub kind: String,
    pub dt_ms: u32,
    pub retention: f64,
    pub recurrence_gain: f64,
    pub noise: f64,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Readout {
    pub approach_count: usize,
    pub avoid_count: usize,
    pub neutral_baseline: [f32; 2],
    pub scale: f32,
    pub calibration: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub backend: String,
    pub rng: String,
    pub source_profile_sha256: String,
    pub exporter_sha256: String,
    pub atlas_sha256: String,
    pub sensor: String,
    pub neural_steps: u32,
    pub physics_ticks: u64,
    pub decision_ms: u32,
    pub nodes: usize,
    pub edges: usize,
    pub readout: Readout,
    pub motor: MotorConfig,
    pub dynamics: Dynamics,
    pub initial_rng: serde_json::Value,
    pub source_lock_sha256: String,
    pub files: BTreeMap<String, String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mapping {
    pub ids: Vec<u64>,
    pub inputs: Vec<usize>,
    pub samples: Vec<[usize; 2]>,
    pub left: Vec<usize>,
    pub right: Vec<usize>,
    pub telemetry: Vec<usize>,
    pub yellow_mask: Vec<bool>,
    pub approach_indices: Vec<usize>,
    pub avoid_indices: Vec<usize>,
}
pub struct Profile {
    pub manifest: Manifest,
    pub mapping: Mapping,
    pub rows: Vec<u32>,
    pub columns: Vec<u32>,
    pub values: Vec<f32>,
    pub anatomy: Vec<usize>,
    pub initial_rng: NumpyRng,
    pub hash: String,
}
fn read(p: &Path, max: u64) -> Result<Vec<u8>, String> {
    if p.metadata().map_err(|e| e.to_string())?.len() > max {
        return Err(format!("Profile file too large: {}", p.display()));
    }
    std::fs::read(p).map_err(|e| e.to_string())
}
impl Profile {
    pub fn load(root: &Path) -> Result<Self, String> {
        let dir = root.join("data/cache/malecns-rust-v1");
        let bytes = read(&dir.join("manifest.json"), 65536)?;
        let m: Manifest = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if m.schema != 1
            || m.backend != BACKEND
            || m.rng != RNG
            || m.sensor != "eye-level-v2 / ChromaticValenceV1"
            || m.neural_steps != wire_types::NEURAL_STEPS
            || m.physics_ticks != wire_types::PHYSICS_TICKS
            || m.decision_ms != 40
            || m.nodes != 164606
            || m.edges != 25558671
            || m.dynamics.dt_ms != 20
            || !m.dynamics.noise.is_finite()
            || !(0.0..=0.01).contains(&m.dynamics.noise)
            || !(0.0..1.0).contains(&m.dynamics.retention)
            || !(0.0..=1.0).contains(&m.dynamics.recurrence_gain)
            || !m.readout.scale.is_finite()
            || m.readout.scale <= 0.
            || m.readout
                .neutral_baseline
                .iter()
                .any(|x| !x.is_finite() || !(0.0..=1.0).contains(x))
        {
            return Err("Incompatible Rust brain profile; run scripts/setup.sh".into());
        }
        m.motor.validate()?;
        if hash(&read(
            &root.join("data/cache/malecns-v1/manifest.json"),
            65536,
        )?) != m.source_profile_sha256
            || hash(&read(&root.join("data/source.lock.json"), 65536)?) != m.source_lock_sha256
            || hash(&read(
                &root.join("controller/brainworker/export_rust.py"),
                65536,
            )?) != m.exporter_sha256
        {
            return Err("Prepared source provenance changed; regenerate Rust profile".into());
        }
        let atlas_bytes = include_bytes!("../../../assets/brain_atlas.json");
        if hash(atlas_bytes) != m.atlas_sha256 {
            return Err("Anatomical atlas/profile mismatch".into());
        }
        let required = ["columns.u32", "mapping.json", "rows.u32", "values.f32"];
        if m.files.keys().map(String::as_str).collect::<Vec<_>>() != required {
            return Err("Unexpected prepared files".into());
        }
        let mut files = BTreeMap::new();
        for name in required {
            let data = read(&dir.join(name), 256_000_000)?;
            if hash(&data) != m.files[name] {
                return Err(format!("Prepared digest mismatch: {name}"));
            }
            files.insert(name, data);
        }
        let mapping: Mapping =
            serde_json::from_slice(&files["mapping.json"]).map_err(|e| e.to_string())?;
        let integers = |name: &str, count: usize| -> Result<Vec<u32>, String> {
            let b = &files[name];
            if b.len() != count * 4 {
                return Err(format!("Invalid array length: {name}"));
            }
            Ok(b.chunks_exact(4)
                .map(|x| u32::from_le_bytes(x.try_into().unwrap()))
                .collect())
        };
        let rows = integers("rows.u32", m.nodes + 1)?;
        let columns = integers("columns.u32", m.edges)?;
        let values: Vec<f32> = integers("values.f32", m.edges)?
            .into_iter()
            .map(f32::from_bits)
            .collect();
        if rows[0] != 0
            || rows[m.nodes] as usize != m.edges
            || rows.windows(2).any(|w| w[0] > w[1])
            || columns.iter().any(|v| *v as usize >= m.nodes)
            || values.iter().any(|v| !v.is_finite() || v.abs() > 1.)
        {
            return Err("Invalid CSR graph".into());
        }
        let population = |v: &[usize]| {
            !v.is_empty()
                && v.iter().all(|i| *i < m.nodes)
                && v.iter().copied().collect::<HashSet<_>>().len() == v.len()
        };
        if mapping.ids.len() != m.nodes
            || mapping.ids.windows(2).any(|w| w[0] >= w[1])
            || [
                &mapping.inputs,
                &mapping.left,
                &mapping.right,
                &mapping.telemetry,
                &mapping.approach_indices,
                &mapping.avoid_indices,
            ]
            .iter()
            .any(|v| !population(v))
            || mapping.inputs.len() != mapping.samples.len()
            || mapping.inputs.len() != mapping.yellow_mask.len()
            || mapping.samples.iter().any(|s| s[0] >= 128 || s[1] >= 96)
            || mapping
                .yellow_mask
                .iter()
                .enumerate()
                .any(|(i, v)| *v != (i % 2 == 0))
            || mapping.approach_indices.len() != m.readout.approach_count
            || mapping.avoid_indices.len() != m.readout.avoid_count
        {
            return Err("Invalid prepared population mapping".into());
        }
        let inputs: HashSet<_> = mapping.inputs.iter().copied().collect();
        let approach: HashSet<_> = mapping.approach_indices.iter().copied().collect();
        if approach.iter().any(|x| inputs.contains(x))
            || mapping
                .avoid_indices
                .iter()
                .any(|x| inputs.contains(x) || approach.contains(x))
        {
            return Err("Readout populations overlap".into());
        }
        #[derive(Deserialize)]
        struct Atlas {
            neurons: Vec<Neuron>,
        }
        #[derive(Deserialize)]
        struct Neuron {
            index: usize,
            body_id: u64,
        }
        let atlas: Atlas = serde_json::from_slice(atlas_bytes).map_err(|e| e.to_string())?;
        if atlas.neurons.len() != 336
            || atlas
                .neurons
                .iter()
                .any(|v| mapping.ids.get(v.index) != Some(&v.body_id))
        {
            return Err("Atlas neurons do not match graph ordering".into());
        }
        let initial_rng = NumpyRng::from_numpy_state(&m.initial_rng)?;
        Ok(Self {
            hash: hash(&bytes),
            manifest: m,
            mapping,
            rows,
            columns,
            values,
            anatomy: atlas.neurons.iter().map(|n| n.index).collect(),
            initial_rng,
        })
    }
}
