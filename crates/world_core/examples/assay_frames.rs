use world_core::*;
fn main() {
    let root = std::path::Path::new("runs/assay-inputs");
    std::fs::create_dir_all(root).unwrap();
    for (i, x) in [16., 32., 48., 80., 96., 112.].into_iter().enumerate() {
        for (j, y) in [210., 242., 260.].into_iter().enumerate() {
            let mut w = World::bridge();
            w.actor.x = 320.;
            w.actor.y = 272.;
            let wx = w.actor.x - 128. + x * 2.;
            w.edit(&Stroke {
                tool: Tool::Ink,
                points: vec![[wx, y]],
                radius: 24.,
                color: [232, 186, 60],
            })
            .unwrap();
            std::fs::write(root.join(format!("cue-{i}-{j}.rgb")), w.observe()).unwrap();
        }
    }
    let w = World::bridge();
    std::fs::write(root.join("initial.rgb"), w.observe()).unwrap();
}
