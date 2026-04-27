//! Inference layer. Phase 3.1 ships a CPU-only synchronous generator;
//! the trait + manager land in 3.2 / 3.3.

pub mod error;
pub mod llama_cpp;

pub use error::InferenceError;
