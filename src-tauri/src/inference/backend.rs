//! Inference backend abstraction (Fase 3.2).
//!
//! Cancellation: drop the `TokenStream`. The producer task observes a closed
//! receiver on its next `blocking_send` and exits without emitting `End`.
//! Cancellation is therefore the *absence* of a final `TokenEvent::End`,
//! not an error.

use std::path::Path;

use async_trait::async_trait;
use tauri::async_runtime::Receiver;

use crate::inference::InferenceError;

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct GenerateParams {
    pub prompt: String,
    /// Total positions (prompt + completion); matches Fase 3.1 `n_len` semantics.
    pub max_tokens: i32,
    pub n_ctx: u32,
    pub seed: u32,
}

impl GenerateParams {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            max_tokens: 128,
            n_ctx: 2048,
            seed: 1234,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TokenEvent {
    Chunk(String),
    End(StopReason),
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum StopReason {
    Eog,
    MaxTokens,
}

#[derive(Debug)]
pub struct TokenStream {
    rx: Receiver<Result<TokenEvent, InferenceError>>,
}

impl TokenStream {
    pub(crate) fn new(rx: Receiver<Result<TokenEvent, InferenceError>>) -> Self {
        Self { rx }
    }

    pub async fn recv(&mut self) -> Option<Result<TokenEvent, InferenceError>> {
        self.rx.recv().await
    }
}

#[async_trait]
pub trait InferenceBackend: Send + Sync {
    async fn load_model(&self, path: &Path) -> Result<(), InferenceError>;
    async fn unload(&self) -> Result<(), InferenceError>;
    async fn generate_stream(&self, params: GenerateParams) -> Result<TokenStream, InferenceError>;
    fn is_loaded(&self) -> bool;
}
