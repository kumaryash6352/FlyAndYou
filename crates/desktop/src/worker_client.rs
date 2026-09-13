use brain_core::Brain;
use fly_brain_worker::session::{Request, Session};
use serde::{Deserialize, Serialize};
use std::{
    cell::Cell,
    io::Write,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
    thread,
    time::{Duration, Instant},
};
use wire_types::{Reply, Step};

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Telemetry {
    #[serde(default)]
    pub activity: Vec<f32>,
    #[serde(default)]
    pub anatomy_activity: Vec<f32>,
    #[serde(default)]
    pub left: f64,
    #[serde(default)]
    pub right: f64,
    #[serde(default)]
    pub input_mean: f64,
    #[serde(default)]
    pub nodes: usize,
    #[serde(default)]
    pub edges: usize,
    #[serde(default)]
    pub ticks: u64,
    #[serde(default)]
    pub steer: f64,
    #[serde(default)]
    pub approach: Option<f64>,
    #[serde(default)]
    pub avoidance: Option<f64>,
    #[serde(default)]
    pub motor_mode: String,
    #[serde(default)]
    pub heading: i8,
}
pub enum Command {
    Step(Step),
    Restore { epoch: String, name: String },
    Snapshot { epoch: String, name: String },
    Stop,
}
pub enum Event {
    Loaded {
        profile: String,
        telemetry: Telemetry,
    },
    Action {
        reply: Reply,
        telemetry: Telemetry,
        ms: f64,
    },
    Restored {
        epoch: String,
        profile: String,
        tick: u64,
        step_id: u64,
        telemetry: Telemetry,
    },
    Saved {
        name: String,
        tick: u64,
    },
    Fault(String),
}
// A timed-out GPU task cannot be killed safely in-process. Keep at most one
// model owner alive; Reset may retry once its current operation has returned.
static ACTIVE_TASK: AtomicBool = AtomicBool::new(false);
struct TaskLease;
impl Drop for TaskLease {
    fn drop(&mut self) {
        ACTIVE_TASK.store(false, Ordering::Release);
    }
}

pub struct Worker {
    tx: SyncSender<Command>,
    rx: Receiver<Event>,
    stop: Arc<AtomicBool>,
    pending: Cell<Option<(Instant, Duration)>>,
    faulted: Cell<bool>,
}
impl Worker {
    pub fn launch(root: &Path, run_name: &str) -> Result<Self, String> {
        if run_name.is_empty()
            || !run_name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("Invalid run name".into());
        }
        std::fs::create_dir_all(root.join("runs")).map_err(|e| e.to_string())?;
        let mut log =
            std::fs::File::create(root.join("runs/worker.log")).map_err(|e| e.to_string())?;
        ACTIVE_TASK
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                "The previous neural task is still stopping. Try Reset again after it returns."
                    .to_string()
            })?;
        let run_dir = root.join("runs").join(run_name);
        let (tx, commands) = mpsc::sync_channel(1);
        let (events, rx) = mpsc::sync_channel(4);
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = Arc::clone(&stop);
        let spawned = thread::Builder::new()
            .name("fly-brain".into())
            .spawn(move || {
                let _lease = TaskLease;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    // Construct and use Candle/Metal exclusively on this task's thread.
                    let mut session = Session::new(Brain::embedded(false)?, run_dir)?;
                    if cancelled.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    let loaded = session.loaded();
                    let profile = loaded["profile_sha256"]
                        .as_str()
                        .ok_or("Missing profile hash")?
                        .to_string();
                    let telemetry = serde_json::from_value(loaded["telemetry"].clone())
                        .map_err(|e| e.to_string())?;
                    events
                        .send(Event::Loaded { profile, telemetry })
                        .map_err(|e| e.to_string())?;
                    while let Ok(command) = commands.recv() {
                        if cancelled.load(Ordering::Acquire) {
                            break;
                        }
                        let event = match command {
                            Command::Step(request) => {
                                let start = Instant::now();
                                let reply = session.step(&request)?;
                                reply.validate(&request)?;
                                let value = session.handle(Request::Inspect {
                                    epoch: request.epoch.clone(),
                                    step_id: request.step_id,
                                })?;
                                if value["kind"] != "inspection"
                                    || value["epoch"] != request.epoch
                                    || value["step_id"] != request.step_id
                                {
                                    return Err("Invalid inspection identity".into());
                                }
                                Event::Action {
                                    reply,
                                    telemetry: serde_json::from_value(value["telemetry"].clone())
                                        .map_err(|e| e.to_string())?,
                                    ms: start.elapsed().as_secs_f64() * 1000.,
                                }
                            }
                            Command::Restore { epoch, name } => {
                                let value = session.handle(Request::Restore {
                                    epoch: epoch.clone(),
                                    checkpoint: name,
                                })?;
                                if value["kind"] != "restored" || value["epoch"] != epoch {
                                    return Err("Invalid restoration acknowledgement".into());
                                }
                                Event::Restored {
                                    epoch,
                                    profile: value["profile_sha256"]
                                        .as_str()
                                        .ok_or("Missing profile")?
                                        .into(),
                                    tick: value["physics_tick"].as_u64().ok_or("Missing tick")?,
                                    step_id: value["next_step_id"]
                                        .as_u64()
                                        .ok_or("Missing step")?,
                                    telemetry: serde_json::from_value(value["telemetry"].clone())
                                        .map_err(|e| e.to_string())?,
                                }
                            }
                            Command::Snapshot { epoch, name } => {
                                let value = session.handle(Request::Save {
                                    epoch,
                                    name: name.clone(),
                                })?;
                                if value["kind"] != "snapshot" || value["name"] != name {
                                    return Err("Checkpoint failed".into());
                                }
                                Event::Saved {
                                    name,
                                    tick: value["physics_tick"].as_u64().ok_or("Missing tick")?,
                                }
                            }
                            Command::Stop => break,
                        };
                        if cancelled.load(Ordering::Acquire) {
                            break;
                        }
                        events.send(event).map_err(|e| e.to_string())?;
                    }
                    Ok::<(), String>(())
                }))
                .unwrap_or_else(|_| {
                    Err("Neural task panicked; Reset can reload the controller.".into())
                });
                if let Err(error) = result {
                    let _ = writeln!(log, "{error}");
                    let _ = events.send(Event::Fault(error));
                }
            });
        if let Err(error) = spawned {
            ACTIVE_TASK.store(false, Ordering::Release);
            return Err(format!("Cannot start neural task: {error}"));
        }
        Ok(Self {
            tx,
            rx,
            stop,
            pending: Cell::new(Some((Instant::now(), Duration::from_secs(30)))),
            faulted: Cell::new(false),
        })
    }
    pub fn send(&self, command: Command) -> Result<(), String> {
        if self.faulted.get() {
            return Err("Neural task has stopped".into());
        }
        if self.pending.get().is_some() {
            return Err("A neural command is already outstanding".into());
        }
        self.tx
            .try_send(command)
            .map_err(|e| format!("Controller queue: {e}"))?;
        self.pending
            .set(Some((Instant::now(), Duration::from_secs(2))));
        Ok(())
    }
    pub fn try_recv(&self) -> Result<Event, TryRecvError> {
        if self.faulted.get() {
            return Err(TryRecvError::Empty);
        }
        match self.rx.try_recv() {
            Ok(event) => {
                self.pending.set(None);
                if matches!(event, Event::Fault(_)) {
                    self.faulted.set(true);
                }
                Ok(event)
            }
            Err(error) => {
                let fault = if error == TryRecvError::Disconnected {
                    Some("Neural task stopped unexpectedly".to_string())
                } else if self
                    .pending
                    .get()
                    .is_some_and(|(since, limit)| since.elapsed() > limit)
                {
                    Some("Neural task timed out; the world is paused. Try Reset once it has stopped.".to_string())
                } else {
                    None
                };
                if let Some(fault) = fault {
                    self.faulted.set(true);
                    self.stop.store(true, Ordering::Release);
                    self.pending.set(None);
                    Ok(Event::Fault(fault))
                } else {
                    Err(error)
                }
            }
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.tx.try_send(Command::Stop);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receive(worker: &Worker) -> Event {
        let since = Instant::now();
        loop {
            match worker.try_recv() {
                Ok(Event::Fault(error)) => panic!("{error}"),
                Ok(event) => return event,
                Err(TryRecvError::Empty) if since.elapsed() < Duration::from_secs(35) => {
                    thread::sleep(Duration::from_millis(2));
                }
                Err(error) => panic!("Embedded task did not respond: {error}"),
            }
        }
    }
    fn step(worker: &Worker, epoch: &str, profile: &str, id: u64) -> (Reply, Telemetry, f64) {
        let rgb: Vec<_> = [195_u8, 80, 57]
            .into_iter()
            .cycle()
            .take(128 * 96 * 3)
            .collect();
        let request = Step::new(epoch.into(), id, id * 4, 1, profile.into(), &rgb);
        worker.send(Command::Step(request)).unwrap();
        match receive(worker) {
            Event::Action {
                reply,
                telemetry,
                ms,
            } => (reply, telemetry, ms),
            _ => panic!("Expected an action"),
        }
    }

    #[test]
    #[ignore = "loads the complete embedded model on Metal; run with --release --ignored"]
    fn embedded_task_replays_checkpoint() {
        let root = std::env::temp_dir().join(format!("fly-embedded-task-{}", std::process::id()));
        let worker = Worker::launch(&root, "verification").unwrap();
        let profile = match receive(&worker) {
            Event::Loaded { profile, telemetry } => {
                assert_eq!(telemetry.nodes, 164606);
                assert_eq!(telemetry.edges, 25558671);
                crate::tutorial_demo::ColorDemos::load(&profile, telemetry.anatomy_activity.len())
                    .unwrap();
                profile
            }
            _ => panic!("Expected model loading"),
        };
        let epoch = "a".repeat(32);
        worker
            .send(Command::Restore {
                epoch: epoch.clone(),
                name: "initial".into(),
            })
            .unwrap();
        assert!(matches!(
            receive(&worker),
            Event::Restored {
                tick: 0,
                step_id: 0,
                ..
            }
        ));
        let mut latencies = Vec::new();
        for id in 0..4 {
            let (_, telemetry, ms) = step(&worker, &epoch, &profile, id);
            assert_eq!(telemetry.ticks, (id + 1) * 2);
            latencies.push(ms);
        }
        worker
            .send(Command::Snapshot {
                epoch: epoch.clone(),
                name: "retreat".into(),
            })
            .unwrap();
        assert!(matches!(receive(&worker), Event::Saved { tick: 16, .. }));
        let expected = step(&worker, &epoch, &profile, 4);
        assert_eq!(expected.1.motor_mode, "retreat");
        let next_epoch = "b".repeat(32);
        worker
            .send(Command::Restore {
                epoch: next_epoch.clone(),
                name: "retreat".into(),
            })
            .unwrap();
        assert!(matches!(
            receive(&worker),
            Event::Restored {
                tick: 16,
                step_id: 4,
                ..
            }
        ));
        let replay = step(&worker, &next_epoch, &profile, 4);
        assert_eq!(
            serde_json::to_value(&expected.0.action).unwrap(),
            serde_json::to_value(&replay.0.action).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&expected.1).unwrap(),
            serde_json::to_value(&replay.1).unwrap()
        );
        for id in 5..105 {
            let (_, telemetry, ms) = step(&worker, &next_epoch, &profile, id);
            assert_eq!(telemetry.ticks, (id + 1) * 2);
            latencies.push(ms);
        }
        latencies.sort_by(f64::total_cmp);
        let report = serde_json::json!({
            "embedded_model": true, "background_thread": true,
            "tutorial_valid": true, "checkpoint_replay_exact": true,
            "decisions": latencies.len() + 2, "profile_sha256": profile,
            "latency_ms": {"median":latencies[latencies.len()/2], "p95":latencies[latencies.len()*95/100], "max":latencies.last()},
        });
        std::fs::write(
            root.join("verification.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        println!("{}\n{}", root.display(), report);
        drop(worker);
        let since = Instant::now();
        while ACTIVE_TASK.load(Ordering::Acquire) && since.elapsed() < Duration::from_secs(3) {
            thread::sleep(Duration::from_millis(2));
        }
        assert!(
            !ACTIVE_TASK.load(Ordering::Acquire),
            "Model task did not stop after drop"
        );
    }
}
