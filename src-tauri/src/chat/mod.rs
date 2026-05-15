//! Chat logic (templates, context window, generation params).

pub mod context;
pub mod generation;
pub mod templates;

pub use context::{fit_prompt_to_context, ContextError};
pub use generation::{
    resolve_max_completion_tokens, resolve_sampling, ChatGenerationOptions, SamplingParams,
};
