use std::{env, fs, path::PathBuf};

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let source = root.join("data/cache/tutorial-rust.json");
    println!("cargo:rerun-if-changed={}", source.display());
    let target = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("tutorial-rust.json");
    fs::copy(&source, target).unwrap_or_else(|error| {
        panic!("Missing embedded tutorial recording: {error}. Run ./scripts/setup.sh to prepare it before building the game.")
    });
}
