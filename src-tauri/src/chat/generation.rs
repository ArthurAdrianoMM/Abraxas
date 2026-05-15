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

use crate::db::conversations::Conversation;

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

/// Resolves the final `SamplingParams` for a generation by layering, in
/// order of decreasing priority:
///
/// 1. Per-call overrides from `ChatGenerationOptions::sampling`.
/// 2. Per-conversation columns persisted on the `conversations` row.
/// 3. `SamplingParams::default()` (matches llama.cpp's vanilla chain).
///
/// Per-call overrides are coarse: today the frontend either provides a full
/// `SamplingParams` or omits it. When omitted, the conversation's stored
/// fields fill in field-by-field. NULL columns fall through to the default.
pub fn resolve_sampling(
    conversation: Option<&Conversation>,
    per_call: Option<SamplingParams>,
) -> SamplingParams {
    if let Some(sampling) = per_call {
        return sampling;
    }
    let mut params = SamplingParams::default();
    let Some(conv) = conversation else {
        return params;
    };
    if let Some(v) = conv.temperature {
        params.temperature = v as f32;
    }
    if let Some(v) = conv.top_p {
        params.top_p = v as f32;
    }
    if let Some(v) = conv.top_k {
        params.top_k = v as i32;
    }
    if let Some(v) = conv.repeat_penalty {
        params.repeat_penalty = v as f32;
    }
    if let Some(v) = conv.repeat_last_n {
        params.repeat_last_n = v as i32;
    }
    if let Some(v) = conv.seed {
        params.seed = v as u32;
    }
    params
}

/// Resolves the max-completion-tokens budget for a generation. Per-call
/// overrides win, then the conversation's stored column, then `None` (the
/// command layer applies `DEFAULT_COMPLETION_BUDGET`).
pub fn resolve_max_completion_tokens(
    conversation: Option<&Conversation>,
    per_call: Option<i32>,
) -> Option<i32> {
    per_call.or_else(|| {
        conversation
            .and_then(|c| c.max_completion_tokens)
            .map(|v| v as i32)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv_with(params: ConversationParams) -> Conversation {
        Conversation {
            id: "id".into(),
            title: "t".into(),
            model_id: None,
            created_at: "now".into(),
            updated_at: "now".into(),
            temperature: params.temperature,
            top_p: params.top_p,
            top_k: params.top_k,
            repeat_penalty: params.repeat_penalty,
            repeat_last_n: params.repeat_last_n,
            seed: params.seed,
            max_completion_tokens: params.max_completion_tokens,
        }
    }

    #[derive(Default)]
    struct ConversationParams {
        temperature: Option<f64>,
        top_p: Option<f64>,
        top_k: Option<i64>,
        repeat_penalty: Option<f64>,
        repeat_last_n: Option<i64>,
        seed: Option<i64>,
        max_completion_tokens: Option<i64>,
    }

    #[test]
    fn resolve_sampling_falls_back_to_default_when_nothing_set() {
        let resolved = resolve_sampling(None, None);
        let d = SamplingParams::default();
        assert_eq!(resolved.temperature, d.temperature);
        assert_eq!(resolved.seed, d.seed);
    }

    #[test]
    fn resolve_sampling_uses_conversation_fields_field_by_field() {
        let conv = conv_with(ConversationParams {
            temperature: Some(0.1),
            seed: Some(42),
            ..Default::default()
        });
        let resolved = resolve_sampling(Some(&conv), None);
        assert!((resolved.temperature - 0.1).abs() < 1e-6);
        assert_eq!(resolved.seed, 42);
        // Untouched fields keep the default.
        assert_eq!(resolved.top_k, SamplingParams::default().top_k);
    }

    #[test]
    fn resolve_sampling_per_call_overrides_conversation() {
        let conv = conv_with(ConversationParams {
            temperature: Some(0.1),
            seed: Some(42),
            ..Default::default()
        });
        let per_call = SamplingParams {
            temperature: 1.5,
            seed: 7,
            ..SamplingParams::default()
        };
        let resolved = resolve_sampling(Some(&conv), Some(per_call));
        assert!((resolved.temperature - 1.5).abs() < 1e-6);
        assert_eq!(resolved.seed, 7);
    }

    #[test]
    fn resolve_max_completion_tokens_layers() {
        let conv = conv_with(ConversationParams {
            max_completion_tokens: Some(128),
            ..Default::default()
        });
        assert_eq!(resolve_max_completion_tokens(None, None), None);
        assert_eq!(resolve_max_completion_tokens(Some(&conv), None), Some(128));
        assert_eq!(
            resolve_max_completion_tokens(Some(&conv), Some(64)),
            Some(64)
        );
    }
}
