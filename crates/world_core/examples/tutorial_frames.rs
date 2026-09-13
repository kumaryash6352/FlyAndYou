use serde_json::json;
use sha2::{Digest, Sha256};

fn main() {
    let frames: Vec<_> = [[195, 80, 57], [232, 186, 60]]
        .into_iter()
        .map(|color| {
            let world = world_core::tutorial_cue(color);
            let rgb = world.observe();
            json!({"color":color, "world_sha256":world.state_hash(),
            "rgb_sha256":format!("{:x}", Sha256::digest(&rgb)), "rgb":rgb})
        })
        .collect();
    println!(
        "{}",
        json!({"sensor":world_core::SENSOR_PROFILE, "scenes":frames})
    );
}
