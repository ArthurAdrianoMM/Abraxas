//! Inference layer.
//!
//! Fase 3.1 shipped a synchronous generator (now folded into `llama_cpp` as a
//! private helper). Fase 3.2 adds the `InferenceBackend` trait plus the first
//! concrete impl with async lifecycle and streamed generation. Fase 3.3 adds
//! the lifecycle manager; 3.4 the cross-platform feature gates; 3.5 the Tauri
//! command layer that emits `TokenEvent`s to the frontend.

pub mod backend;
pub mod error;
pub mod llama_cpp;
pub mod manager;

pub use backend::{GenerateParams, InferenceBackend, StopReason, TokenEvent, TokenStream};
pub use error::InferenceError;
pub use llama_cpp::LlamaCppBackend;
pub use manager::{LoadedModel, ModelManager};
