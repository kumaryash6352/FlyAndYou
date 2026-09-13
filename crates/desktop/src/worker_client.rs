use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader},
    net::TcpStream,
    path::Path,
    process::{Child, Command as ProcessCommand, Stdio},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};
use wire_types::{Reply, Step, read_frame, write_frame};
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
    pub approach: f64,
    #[serde(default)]
    pub avoidance: f64,
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
pub struct Worker {
    pub tx: SyncSender<Command>,
    pub rx: Receiver<Event>,
    child: Arc<Mutex<Child>>,
}
impl Worker {
    pub fn launch(root: &Path, run_name: &str) -> Result<Self, String> {
        std::fs::create_dir_all(root.join("runs")).map_err(|e| e.to_string())?;
        let log = std::fs::File::create(root.join("runs/worker.log")).map_err(|e| e.to_string())?;
        let mut child = ProcessCommand::new(root.join(".venv/bin/python"))
            .current_dir(root)
            .args([
                "-m",
                "controller.brainworker.service",
                "--runs",
                &format!("runs/{run_name}"),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::from(log))
            .spawn()
            .map_err(|e| format!("Cannot start neural worker: {e}. Run scripts/setup.sh."))?;
        let stdout = child.stdout.take().ok_or("Missing worker output")?;
        let child = Arc::new(Mutex::new(child));
        let (tx, commands) = mpsc::sync_channel(1);
        let (events, rx) = mpsc::sync_channel(4);
        thread::spawn(move || {
            let result = (|| -> Result<(), String> {
                let mut line = String::new();
                BufReader::new(stdout)
                    .read_line(&mut line)
                    .map_err(|e| e.to_string())?;
                if line.len() > 4096 {
                    return Err("Invalid worker startup".into());
                }
                let port: Value = serde_json::from_str(&line)
                    .map_err(|_| "Worker failed to start; see runs/worker.log")?;
                let port = port["port"]
                    .as_u64()
                    .filter(|p| *p > 0 && *p <= 65535)
                    .ok_or("Invalid local port")?;
                let mut socket =
                    TcpStream::connect(("127.0.0.1", port as u16)).map_err(|e| e.to_string())?;
                socket.set_nodelay(true).map_err(|e| e.to_string())?;
                socket
                    .set_read_timeout(Some(Duration::from_secs(30)))
                    .map_err(|e| e.to_string())?;
                socket
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .map_err(|e| e.to_string())?;
                let loaded: Value = read_frame(&mut socket)?;
                if loaded["kind"] != "loaded" {
                    return Err("Neural profile failed to load".into());
                }
                let profile = loaded["profile_sha256"]
                    .as_str()
                    .ok_or("Missing profile hash")?
                    .to_string();
                let telemetry = serde_json::from_value(loaded["telemetry"].clone())
                    .map_err(|e| e.to_string())?;
                events
                    .send(Event::Loaded { profile, telemetry })
                    .map_err(|e| e.to_string())?;
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .map_err(|e| e.to_string())?;
                while let Ok(cmd) = commands.recv() {
                    match cmd {
                        Command::Step(r) => {
                            let start = Instant::now();
                            write_frame(&mut socket, &r)?;
                            let reply: Reply = read_frame(&mut socket)?;
                            reply.validate(&r)?;
                            write_frame(
                                &mut socket,
                                &json!({"kind":"inspect","epoch":r.epoch,"step_id":r.step_id}),
                            )?;
                            let v: Value = read_frame(&mut socket)?;
                            if v["kind"] != "inspection"
                                || v["epoch"] != r.epoch
                                || v["step_id"] != r.step_id
                            {
                                return Err("Invalid inspection identity".into());
                            }
                            let telemetry = serde_json::from_value(v["telemetry"].clone())
                                .map_err(|e| e.to_string())?;
                            events
                                .send(Event::Action {
                                    reply,
                                    telemetry,
                                    ms: start.elapsed().as_secs_f64() * 1000.,
                                })
                                .map_err(|e| e.to_string())?;
                        }
                        Command::Restore { epoch, name } => {
                            write_frame(
                                &mut socket,
                                &json!({"kind":"restore","epoch":epoch,"checkpoint":name}),
                            )?;
                            let v: Value = read_frame(&mut socket)?;
                            if v["kind"] != "restored" || v["epoch"] != epoch {
                                return Err("Invalid restoration acknowledgement".into());
                            }
                            events
                                .send(Event::Restored {
                                    epoch,
                                    profile: v["profile_sha256"]
                                        .as_str()
                                        .ok_or("Missing profile")?
                                        .into(),
                                    tick: v["physics_tick"].as_u64().ok_or("Missing tick")?,
                                    step_id: v["next_step_id"].as_u64().ok_or("Missing step")?,
                                    telemetry: serde_json::from_value(v["telemetry"].clone())
                                        .map_err(|e| e.to_string())?,
                                })
                                .map_err(|e| e.to_string())?;
                        }
                        Command::Snapshot { epoch, name } => {
                            write_frame(
                                &mut socket,
                                &json!({"kind":"snapshot","epoch":epoch,"name":name}),
                            )?;
                            let v: Value = read_frame(&mut socket)?;
                            if v["kind"] != "snapshot" || v["name"] != name {
                                return Err("Checkpoint failed".into());
                            }
                            events
                                .send(Event::Saved {
                                    name,
                                    tick: v["physics_tick"].as_u64().ok_or("Missing tick")?,
                                })
                                .map_err(|e| e.to_string())?;
                        }
                        Command::Stop => break,
                    }
                }
                Ok(())
            })();
            if let Err(e) = result {
                let _ = events.send(Event::Fault(e));
            }
        });
        Ok(Self { tx, rx, child })
    }
    pub fn send(&self, c: Command) -> Result<(), String> {
        self.tx
            .try_send(c)
            .map_err(|e| format!("Controller queue: {e}"))
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.tx.try_send(Command::Stop);
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
