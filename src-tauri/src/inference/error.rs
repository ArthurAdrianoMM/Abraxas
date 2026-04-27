//! Inference-specific errors. Phase 3.5 will add an `AppError::Inference`
//! variant that wraps this type so the Tauri command layer can surface it
//! through `CommandError`. Until then this stays standalone.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum InferenceError {
    #[error("model file not found: {0}")]
    ModelNotFound(PathBuf),

    #[error("llama backend error: {0}")]
    Backend(#[from] llama_cpp_2::LlamaCppError),

    #[error("model load failed: {0}")]
    ModelLoad(#[from] llama_cpp_2::LlamaModelLoadError),

    #[error("context creation failed: {0}")]
    ContextCreate(#[from] llama_cpp_2::LlamaContextLoadError),

    #[error("tokenize failed: {0}")]
    Tokenize(#[from] llama_cpp_2::StringToTokenError),

    #[error("decode failed: {0}")]
    Decode(#[from] llama_cpp_2::DecodeError),

    #[error("token-to-string failed: {0}")]
    Detokenize(#[from] llama_cpp_2::TokenToStringError),

    #[error("batch add failed: {0}")]
    BatchAdd(#[from] llama_cpp_2::llama_batch::BatchAddError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
