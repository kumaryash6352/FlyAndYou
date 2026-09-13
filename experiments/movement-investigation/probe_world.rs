// Investigation-only driver. World metadata is returned to the harness, never Brain.
use world_core::*;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
fn main() {
    let mut w = World::bridge();
    for line in io::stdin().lock().lines() {
        let r: Value = serde_json::from_str(&line.unwrap()).unwrap();
        match r["kind"].as_str().unwrap() {
            "reset" => {
                w = World::bridge();
                w.actor.x = r["x"].as_f64().unwrap_or(320.);
                w.facing = r["facing"].as_i64().unwrap_or(1) as i8;
                if r["bridge"].as_bool().unwrap_or(false) {
                    w.edit(&Stroke {tool: Tool::Solid, points: vec![[188.,282.],[292.,282.]], radius:4., color:[0;3]}).unwrap();
                }
            }
            "edit" => {
                let tool = match r["tool"].as_str().unwrap() {"solid" => Tool::Solid, "erase_ink" => Tool::EraseInk, _ => Tool::Ink};
                w.edit(&Stroke {tool, points:serde_json::from_value(r["points"].clone()).unwrap(), radius:r["radius"].as_f64().unwrap_or(24.), color:serde_json::from_value(r["color"].clone()).unwrap_or([232,186,60])}).unwrap();
            }
            "step" => {for _ in 0..10 {w.step(Action{steer:r["steer"].as_f64().unwrap(),jump:false});}}
            "observe" => {},
            _ => panic!("Unknown command"),
        }
        println!("{}", json!({"x":w.actor.x,"y":w.actor.y,"vx":w.actor.vx,"facing":w.facing,"tick":w.tick,"outcome":format!("{:?}",w.outcome),"rgb":w.observe()}));
        io::stdout().flush().unwrap();
    }
}
