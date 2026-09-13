use brain_core::{Brain, Snapshot};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};
use wire_types::{Reply, Step, hash, valid_hex};

#[derive(Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Request {
    #[serde(rename = "step")]
    Step {
        version: u32,
        epoch: String,
        step_id: u64,
        physics_tick: u64,
        world_revision: u64,
        profile_sha256: String,
        rgb_sha256: String,
        width: u32,
        height: u32,
        format: String,
        neural_steps: u32,
        frame_b64: String,
    },
    #[serde(rename = "restore")]
    Restore { epoch: String, checkpoint: String },
    #[serde(rename = "inspect")]
    Inspect { epoch: String, step_id: u64 },
    #[serde(rename = "snapshot")]
    Save { epoch: String, name: String },
    #[serde(rename = "shutdown")]
    Shutdown { epoch: String },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bookmark {
    schema: u32,
    profile_sha256: String,
    next_id: u64,
    state: Snapshot,
}
pub struct Session {
    pub brain: Brain,
    initial: Snapshot,
    root: PathBuf,
    epoch: Option<String>,
    next_id: u64,
    cached: Option<(u64, String, Reply)>,
    elapsed_ms: f64,
}
impl Session {
    pub fn new(brain: Brain, root: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let initial = brain.snapshot();
        Ok(Self {
            brain,
            initial,
            root,
            epoch: None,
            next_id: 0,
            cached: None,
            elapsed_ms: 0.,
        })
    }
    fn checkpoint_path(&self, name: &str) -> Result<PathBuf, String> {
        if name.is_empty()
            || name.len() > 80
            || !name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            return Err("Invalid checkpoint name".into());
        }
        Ok(self.root.join(format!("{name}.brain.json")))
    }
    fn require_epoch(&self, e: &str) -> Result<(), String> {
        if self.epoch.as_deref() != Some(e) {
            Err("Stale epoch".into())
        } else {
            Ok(())
        }
    }
    pub fn loaded(&self) -> Value {
        json!({"kind":"loaded","profile_sha256":self.brain.profile_hash,"mode":"MaleCNS fixed controller","nodes":self.brain.activity.len(),"edges":self.brain.profile.values.len(),"telemetry":self.brain.inspect()})
    }
    /// Shared typed path for the game's task and the optional socket reference tool.
    pub fn step(&mut self, r: &Step) -> Result<Reply, String> {
        let rgb = r.validate()?;
        if r.profile_sha256 != self.brain.profile_hash {
            return Err("Wrong profile".into());
        }
        if let Some(e) = &self.epoch {
            if e != &r.epoch {
                return Err("Stale epoch".into());
            }
        }
        let digest = hash(&serde_json::to_vec(&r).map_err(|e| e.to_string())?);
        if let Some((id, old, reply)) = &self.cached {
            if *id == r.step_id {
                if *old != digest {
                    return Err("Duplicate id with changed content".into());
                }
                return Ok(reply.clone());
            }
        }
        if r.step_id != self.next_id || self.brain.ticks.checked_mul(2) != Some(r.physics_tick) {
            return Err("Unexpected decision boundary".into());
        }
        let start = Instant::now();
        let steer = self.brain.step_image(&rgb)?;
        self.elapsed_ms = start.elapsed().as_secs_f64() * 1000.;
        let reply = Reply::for_request(&r, steer);
        reply.validate(&r)?;
        self.next_id = self.next_id.checked_add(1).ok_or("Step overflow")?;
        self.epoch = Some(r.epoch.clone());
        self.cached = Some((r.step_id, digest, reply.clone()));
        Ok(reply)
    }
    pub fn handle(&mut self, request: Request) -> Result<Value, String> {
        match request {
            Request::Step {
                version,
                epoch,
                step_id,
                physics_tick,
                world_revision,
                profile_sha256,
                rgb_sha256,
                width,
                height,
                format,
                neural_steps,
                frame_b64,
            } => {
                let r = Step {
                    kind: "step".into(),
                    version,
                    epoch,
                    step_id,
                    physics_tick,
                    world_revision,
                    profile_sha256,
                    rgb_sha256,
                    width,
                    height,
                    format,
                    neural_steps,
                    frame_b64,
                };
                serde_json::to_value(self.step(&r)?).map_err(|e| e.to_string())
            }
            Request::Restore { epoch, checkpoint } => {
                if !valid_hex(&epoch, 32) {
                    return Err("Invalid epoch".into());
                }
                let (state, next_id) = if checkpoint == "initial" {
                    (self.initial.clone(), 0)
                } else {
                    let p = self.checkpoint_path(&checkpoint)?;
                    if p.metadata().map_err(|e| e.to_string())?.len() > 2_000_000 {
                        return Err("Checkpoint too large".into());
                    }
                    let saved: Bookmark =
                        serde_json::from_slice(&std::fs::read(p).map_err(|e| e.to_string())?)
                            .map_err(|e| e.to_string())?;
                    if saved.schema != 1
                        || saved.profile_sha256 != self.brain.profile_hash
                        || saved.next_id.checked_mul(wire_types::NEURAL_STEPS as u64)
                            != Some(saved.state.ticks)
                    {
                        return Err("Incompatible checkpoint boundary".into());
                    }
                    (saved.state, saved.next_id)
                };
                self.brain.restore(&state)?;
                self.next_id = next_id;
                self.epoch = Some(epoch.clone());
                self.cached = None;
                self.elapsed_ms = 0.;
                Ok(
                    json!({"kind":"restored","epoch":epoch,"profile_sha256":self.brain.profile_hash,"next_step_id":self.next_id,"physics_tick":self.brain.ticks*2,"telemetry":self.brain.inspect()}),
                )
            }
            Request::Inspect { epoch, step_id } => {
                self.require_epoch(&epoch)?;
                if self.next_id.checked_sub(1) != Some(step_id) {
                    return Err("Inspection boundary mismatch".into());
                }
                Ok(
                    json!({"kind":"inspection","epoch":epoch,"step_id":step_id,"elapsed_ms":self.elapsed_ms,"telemetry":self.brain.inspect()}),
                )
            }
            Request::Save { epoch, name } => {
                self.require_epoch(&epoch)?;
                let p = self.checkpoint_path(&name)?;
                let saved = Bookmark {
                    schema: 1,
                    profile_sha256: self.brain.profile_hash.clone(),
                    next_id: self.next_id,
                    state: self.brain.snapshot(),
                };
                let blob = serde_json::to_vec(&saved).map_err(|e| e.to_string())?;
                atomic_write(&p, &blob)?;
                Ok(
                    json!({"kind":"snapshot","name":name,"sha256":hash(&blob),"physics_tick":self.brain.ticks*2}),
                )
            }
            Request::Shutdown { epoch } => {
                self.require_epoch(&epoch)?;
                Ok(json!({"kind":"shutdown"}))
            }
        }
    }
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(temp, path).map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_commands_reject_duplicates_and_unknown_fields() {
        assert!(
            serde_json::from_str::<Request>(
                r#"{"kind":"inspect","epoch":"a","step_id":0,"step_id":1}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<Request>(r#"{"kind":"shutdown","epoch":"a","goal":42}"#)
                .is_err()
        );
    }
}
