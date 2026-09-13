use brain_core::Brain;
use fly_brain_worker::session;
use session::{Request, Session};
use std::{
    io::{BufReader, Write},
    net::TcpListener,
    path::PathBuf,
    time::{Duration, Instant},
};
use wire_types::{read_frame, write_frame};

fn run() -> Result<(), String> {
    let mut root = std::env::var_os("FLY_AND_YOU_ROOT")
        .map(PathBuf::from)
        .unwrap_or(std::env::current_dir().map_err(|e| e.to_string())?);
    let mut runs = "runs/live-rust".to_string();
    let mut cpu = false;
    let mut tutorial = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = PathBuf::from(args.next().ok_or("Missing root")?),
            "--runs" => runs = args.next().ok_or("Missing runs directory")?,
            "--backend" => {
                let name = args.next().ok_or("Missing backend")?;
                cpu = match name.as_str() {
                    "cpu" => true,
                    "metal" => false,
                    _ => return Err("Backend must be cpu or metal".into()),
                };
            }
            "--prepare-tutorial" => tutorial = true,
            _ => return Err(format!("Unknown option {arg}")),
        }
    }
    root = root.canonicalize().map_err(|e| e.to_string())?;
    if tutorial {
        return prepare_tutorial(&root);
    }
    let runs = PathBuf::from(runs);
    if runs.is_absolute()
        || !runs.starts_with("runs")
        || runs
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err("Run directory must be inside workspace runs".into());
    }
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| e.to_string())?;
    println!(
        "{}",
        serde_json::json!({"port":listener.local_addr().map_err(|e|e.to_string())?.port()})
    );
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let mut session = Session::new(Brain::load(&root, cpu)?, root.join(runs))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let since = Instant::now();
    let mut socket = loop {
        match listener.accept() {
            Ok((s, _)) => break s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if since.elapsed() > Duration::from_secs(30) {
                    return Err("No host connected".into());
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    // Accepted sockets inherit nonblocking mode on macOS; framed reads must block.
    socket.set_nonblocking(false).map_err(|e| e.to_string())?;
    socket.set_nodelay(true).map_err(|e| e.to_string())?;
    socket
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    write_frame(&mut socket, &session.loaded())?;
    let mut reader = BufReader::new(socket.try_clone().map_err(|e| e.to_string())?);
    loop {
        let result = read_frame::<_, Request>(&mut reader).and_then(|r| session.handle(r));
        match result {
            Ok(reply) => {
                write_frame(&mut socket, &reply)?;
                if reply["kind"] == "shutdown" {
                    return Ok(());
                }
            }
            Err(e) => {
                let _ = write_frame(&mut socket, &serde_json::json!({"kind":"fault","error":e}));
                return Err(e);
            }
        }
    }
}
fn prepare_tutorial(root: &std::path::Path) -> Result<(), String> {
    let mut brain = Brain::load(root, false)?;
    let initial = brain.snapshot();
    let scenes = world_core::tutorial_cue;
    let sources = fly_brain_worker::tutorial_source_hashes();
    let mut identity = serde_json::json!({"schema":2,"sensor":world_core::SENSOR_PROFILE,"profile_sha256":brain.profile_hash,"sources":sources,"seed":7,"seconds_per_sample":0.2});
    let output = root.join("data/cache/tutorial-rust.json");
    let worlds = [[195, 80, 57], [232, 186, 60]].map(scenes);
    if let Ok(old) = std::fs::read(&output).and_then(|b| {
        serde_json::from_slice::<serde_json::Value>(&b).map_err(std::io::Error::other)
    }) {
        if identity
            .as_object()
            .unwrap()
            .iter()
            .all(|(k, v)| old.get(k) == Some(v))
            && old["scenes"].as_array().is_some_and(|ss| {
                ss.len() == 2
                    && ss.iter().zip(&worlds).all(|(s, w)| {
                        s["world_sha256"] == w.state_hash()
                            && s["rgb_sha256"] == wire_types::hash(&w.observe())
                    })
            })
        {
            println!("Rust tutorial color responses are current.");
            return Ok(());
        }
    }
    let mut recorded = Vec::new();
    for (world, mode) in worlds.iter().zip(["retreat", "approach"]) {
        brain.restore(&initial)?;
        let rgb = world.observe();
        let mut samples = vec![brain.inspect()];
        let mut observed = false;
        // Twenty intervals of 0.2 s; five 40 ms decisions per displayed sample.
        for _ in 0..20 {
            for _ in 0..5 {
                brain.step_image(&rgb)?;
                observed |= brain.motor.mode == mode;
            }
            samples.push(brain.inspect());
        }
        if !observed {
            return Err(format!("Tutorial did not evoke {mode}"));
        }
        recorded.push(serde_json::json!({"color":if mode=="retreat"{[195,80,57]}else{[232,186,60]},"world_sha256":world.state_hash(),"rgb_sha256":wire_types::hash(&rgb),"samples":samples}));
    }
    identity["scenes"] = recorded.into();
    session::atomic_write(
        &output,
        &serde_json::to_vec(&identity).map_err(|e| e.to_string())?,
    )?;
    println!("Recorded actual Rust/Metal tutorial responses.");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
