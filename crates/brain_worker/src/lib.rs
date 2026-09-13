pub mod session;

/// Compiled inputs used to validate the recorded tutorial without source-file access.
pub fn tutorial_source_hashes() -> std::collections::BTreeMap<String, String> {
    let mut sources = brain_core::source_hashes();
    sources.insert(
        "crates/brain_worker/src/main.rs".into(),
        wire_types::hash(include_bytes!("main.rs")),
    );
    sources
}
