use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_EMBEDDED_MODEL");
    if env::var_os("CARGO_FEATURE_EMBEDDED_MODEL").is_none() {
        return;
    }
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let mut code = String::from(
        "pub fn asset(name: &str) -> Result<&'static [u8], String> { Ok(match name {\n",
    );
    for name in [
        "data/cache/malecns-rust-v1/manifest.json",
        "data/cache/malecns-rust-v1/columns.u32",
        "data/cache/malecns-rust-v1/rows.u32",
        "data/cache/malecns-rust-v1/values.f32",
        "data/cache/malecns-rust-v1/mapping.json",
        "data/cache/malecns-v1/manifest.json",
        "data/source.lock.json",
        "controller/brainworker/export_rust.py",
    ] {
        let path = root.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        let path = path.canonicalize().unwrap_or_else(|_| {
            panic!("Missing embedded model asset {name}. Run ./scripts/setup.sh to prepare the model before building the game.")
        });
        code.push_str(&format!("{name:?} => include_bytes!({path:?}),\n"));
    }
    code.push_str("_ => return Err(format!(\"Unknown embedded model asset: {name}\")),\n}) }\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("embedded_model.rs"),
        code,
    )
    .unwrap();
}
