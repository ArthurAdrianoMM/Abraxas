//! User-facing generation parameters (Fase 5.4).
//!
//! `SamplingParams` captures the per-conversation knobs the UI exposes:
//! temperature, top-p/top-k, and repetition penalty. The defaults match
//! llama.cpp's own defaults so a fresh conversation behaves like a "vanilla"
//! llama.cpp run. `ChatGenerationOptions` bundles those with the harder
//! limits (max_tokens, n_ctx) that the command layer derives from the loaded
//! model and the catalog entry.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
pub struct SamplingParams {
    /// 0.0 = greedy decoding (deterministic). Otherwise scales the logits
    /// before sampling. llama.cpp default is 0.8.
    pub temperature: f32,
    /// Nucleus sampling cutoff. 1.0 = disabled.
    pub top_p: f32,
    /// Top-k cutoff. 0 = disabled.
    pub top_k: i32,
    /// Repetition penalty. 1.0 = disabled.
    pub repeat_penalty: f32,
    /// How many recent tokens the repeat penalty considers. 0 = disabled,
    /// negative = full context.
    pub repeat_last_n: i32,
    /// RNG seed for `dist`-stage sampling.
    pub seed: u32,
}

impl Default for SamplingParams {
    fn default() -> Self {
        // Matches llama.cpp's default sampling chain.
        Self {
            temperature: 0.8,
            top_p: 0.95,
            top_k: 40,
            repeat_penalty: 1.1,
            repeat_last_n: 64,
            seed: 1234,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Default)]
pub struct ChatGenerationOptions {
    /// Maximum number of completion tokens to emit. `None` defers to a
    /// backend default.
    pub max_completion_tokens: Option<i32>,
    /// Sampling parameters. `None` uses `SamplingParams::default()`.
    pub sampling: Option<SamplingParams>,
}
