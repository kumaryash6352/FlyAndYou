use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use world_core::*;
fn main() {
    let mut w = World::bridge();
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let response = (|| -> Result<Value, String> {
            let r: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
            match r["kind"].as_str().unwrap_or("") {
                "reset" => w = World::bridge(),
                "step" => {
                    let steer = r["steer"].as_f64().ok_or("steer required")?;
                    for _ in 0..10 {
                        let was_running = w.outcome == Outcome::Running;
                        w.step(Action { steer, jump: false });
                        if !was_running {
                            w.tick += 1;
                        }
                    }
                }
                "edit" => {
                    let points: Vec<[f64; 2]> =
                        serde_json::from_value(r["points"].clone()).map_err(|e| e.to_string())?;
                    let tool = match r["tool"].as_str() {
                        Some("solid") => Tool::Solid,
                        Some("ink") => Tool::Ink,
                        Some("erase_ink") => Tool::EraseInk,
                        _ => return Err("Unknown tool".into()),
                    };
                    w.edit(&Stroke {
                        tool,
                        points,
                        radius: r["radius"].as_f64().unwrap_or(24.),
                        color: [232, 186, 60],
                    })?;
                }
                "observe" => {}
                _ => return Err("Unknown request".into()),
            };
            Ok(
                json!({"x":w.actor.x,"y":w.actor.y,"tick":w.tick,"revision":w.revision,"outcome":format!("{:?}",w.outcome),"rgb":w.observe(),"state_hash":w.state_hash()}),
            )
        })();
        match response {
            Ok(v) => println!("{v}"),
            Err(e) => println!("{}", json!({"error":e})),
        }
        io::stdout().flush().unwrap();
    }
}
