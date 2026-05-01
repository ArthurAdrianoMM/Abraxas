//! Chat logic (templates, context window, generation params).

pub mod context;
pub mod generation;
pub mod templates;

pub use context::truncate_to_fit;
pub use generation::{ChatGenerationOptions, SamplingParams};
pub use templates::{
    bos_policy_for, render_chat_template, render_chat_template_with_options, BosPolicy,
    ChatMessage, ChatRole, RenderOptions, TemplateError,
};
