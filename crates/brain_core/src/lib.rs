#[cfg(feature = "embedded-model")]
mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded_model.rs"));
}
mod backend;
mod model;
pub mod motor;
pub mod profile;
pub mod rng;
pub use model::{Brain, Snapshot, source_hashes};
