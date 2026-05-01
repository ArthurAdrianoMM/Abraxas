//! Chat logic (templates, context window, generation params).

pub mod context;
pub mod generation;
pub mod templates;

pub use context::truncate_to_fit;
pub use generation::{ChatGenerationOptions, SamplingParams};
