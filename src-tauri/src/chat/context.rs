//! Context-window fitting (Fase 5.3).
//!
//! When a conversation grows past the loaded model's context window, we drop
//! the oldest non-system messages until the rendered prompt fits, while always
//! preserving every system message and the latest message. System messages
//! carry persona and behavior instructions; losing them silently changes model
//! behavior in ways the user cannot see.

use std::future::Future;

use thiserror::Error;

use crate::chat::templates::{
    render_chat_template_with_options, ChatMessage, ChatRole, RenderOptions, TemplateError,
};
use crate::inference::InferenceError;
use crate::models::catalog::ChatTemplate;

/// Reserve this many tokens of headroom on top of the completion budget for
/// template/tokenizer boundary differences and final control tokens.
const SAFETY_MARGIN_TOKENS: u32 = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FittedPrompt {
    pub messages: Vec<ChatMessage>,
    pub prompt: String,
    pub prompt_tokens: usize,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContextError {
    #[error(transparent)]
    Template(#[from] TemplateError),

    #[error("token counting failed: {0}")]
    TokenCount(String),

    #[error(
        "prompt requires {prompt_tokens} tokens but only {max_prompt_tokens} are available after reserving completion budget"
    )]
    PromptTooLarge {
        prompt_tokens: usize,
        max_prompt_tokens: usize,
    },
}

/// Render and trim `messages` until the exact tokenizer-backed prompt count
/// fits inside `n_ctx - completion_budget - SAFETY_MARGIN_TOKENS`.
///
/// Performance: under pressure this calls `count_tokens` once per older
/// non-system message considered (newest-first), each tokenizing the
/// then-current rendered prompt. That's O(N) tokenizer round-trips for a
/// conversation of N messages. Fine for typical chat depth; revisit with a
/// binary search if profiling ever shows this on the hot path.
pub async fn fit_prompt_to_context<F, Fut>(
    template: ChatTemplate,
    messages: &[ChatMessage],
    n_ctx: u32,
    completion_budget: u32,
    mut count_tokens: F,
) -> Result<FittedPrompt, ContextError>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<usize, InferenceError>>,
{
    let max_prompt_tokens = n_ctx
        .saturating_sub(completion_budget)
        .saturating_sub(SAFETY_MARGIN_TOKENS) as usize;

    let full = render_and_count(template, messages.to_vec(), &mut count_tokens).await?;
    if full.prompt_tokens <= max_prompt_tokens {
        return Ok(full);
    }

    let total = messages.len();

    let last_idx = messages.len().saturating_sub(1);
    let mut keep = vec![false; messages.len()];
    for (i, message) in messages.iter().enumerate() {
        if message.role == ChatRole::System || i == last_idx {
            keep[i] = true;
        }
    }

    let mut best =
        render_and_count(template, kept_messages(messages, &keep), &mut count_tokens).await?;
    if best.prompt_tokens > max_prompt_tokens {
        return Err(ContextError::PromptTooLarge {
            prompt_tokens: best.prompt_tokens,
            max_prompt_tokens,
        });
    }

    for i in (0..last_idx).rev() {
        if keep[i] {
            continue;
        }
        let mut trial_keep = keep.clone();
        trial_keep[i] = true;
        let trial = render_and_count(
            template,
            kept_messages(messages, &trial_keep),
            &mut count_tokens,
        )
        .await?;
        if trial.prompt_tokens <= max_prompt_tokens {
            keep = trial_keep;
            best = trial;
        }
    }

    let kept = keep.iter().filter(|k| **k).count();
    tracing::debug!(
        dropped = total - kept,
        kept,
        prompt_tokens = best.prompt_tokens,
        budget = max_prompt_tokens,
        "context window: truncated conversation to fit"
    );

    Ok(best)
}

async fn render_and_count<F, Fut>(
    template: ChatTemplate,
    messages: Vec<ChatMessage>,
    count_tokens: &mut F,
) -> Result<FittedPrompt, ContextError>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<usize, InferenceError>>,
{
    let prompt = render_chat_template_with_options(template, &messages, RenderOptions::default())?;
    let prompt_tokens = count_tokens(prompt.clone())
        .await
        .map_err(|e| ContextError::TokenCount(e.to_string()))?;

    Ok(FittedPrompt {
        messages,
        prompt,
        prompt_tokens,
    })
}

fn kept_messages(messages: &[ChatMessage], keep: &[bool]) -> Vec<ChatMessage> {
    messages
        .iter()
        .zip(keep.iter())
        .filter(|(_, keep)| **keep)
        .map(|(message, _)| message.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::catalog::ChatTemplate;

    fn msg(role: ChatRole, content: &str) -> ChatMessage {
        ChatMessage::new(role, content)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn keeps_all_when_rendered_prompt_fits() {
        let msgs = vec![
            msg(ChatRole::System, "be terse"),
            msg(ChatRole::User, "hi"),
            msg(ChatRole::Assistant, "hello"),
            msg(ChatRole::User, "bye"),
        ];
        let fitted = fit_prompt_to_context(
            ChatTemplate::ChatML,
            &msgs,
            4096,
            256,
            |prompt| async move { Ok(prompt.len() / 4) },
        )
        .await
        .unwrap();

        assert_eq!(fitted.messages, msgs);
        assert!(fitted.prompt.contains("<|im_start|>system\nbe terse"));
        assert!(fitted.prompt.contains("<|im_start|>user\nbye"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn drops_oldest_non_system_messages_first() {
        let msgs = vec![
            msg(ChatRole::System, "system prompt"),
            msg(ChatRole::User, "old-user"),
            msg(ChatRole::Assistant, "old-assistant"),
            msg(ChatRole::User, "recent-user"),
            msg(ChatRole::User, "current"),
        ];
        let fitted =
            fit_prompt_to_context(ChatTemplate::ChatML, &msgs, 128, 32, |prompt| async move {
                if prompt.contains("old-user") || prompt.contains("old-assistant") {
                    Ok(1_000)
                } else {
                    Ok(40)
                }
            })
            .await
            .unwrap();

        assert!(fitted.messages.iter().any(|m| m.role == ChatRole::System));
        assert!(fitted.messages.iter().any(|m| m.content == "recent-user"));
        assert_eq!(fitted.messages.last().unwrap().content, "current");
        assert!(!fitted.messages.iter().any(|m| m.content == "old-user"));
        assert!(!fitted.messages.iter().any(|m| m.content == "old-assistant"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn always_keeps_system_messages_under_pressure() {
        let msgs = vec![
            msg(ChatRole::System, "primary system"),
            msg(ChatRole::User, "old-user"),
            msg(ChatRole::System, "late system"),
            msg(ChatRole::Assistant, "old-assistant"),
            msg(ChatRole::User, "tail"),
        ];
        let fitted =
            fit_prompt_to_context(ChatTemplate::ChatML, &msgs, 96, 32, |prompt| async move {
                if prompt.contains("old-user") || prompt.contains("old-assistant") {
                    Ok(1_000)
                } else {
                    Ok(32)
                }
            })
            .await
            .unwrap();

        assert!(fitted
            .messages
            .iter()
            .any(|m| m.content == "primary system"));
        assert!(fitted.messages.iter().any(|m| m.content == "late system"));
        assert_eq!(fitted.messages.last().unwrap().content, "tail");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn errors_when_system_and_latest_message_cannot_fit() {
        let msgs = vec![
            msg(ChatRole::System, "system prompt"),
            msg(ChatRole::User, "old-user"),
            msg(ChatRole::User, "current"),
        ];

        let err = fit_prompt_to_context(ChatTemplate::ChatML, &msgs, 128, 32, |_prompt| async {
            Ok(100)
        })
        .await
        .unwrap_err();

        assert_eq!(
            err,
            ContextError::PromptTooLarge {
                prompt_tokens: 100,
                max_prompt_tokens: 64
            }
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn empty_input_returns_template_error() {
        let err = fit_prompt_to_context(ChatTemplate::ChatML, &[], 4096, 256, |_prompt| async {
            Ok(0)
        })
        .await
        .unwrap_err();

        assert!(matches!(err, ContextError::Template(_)));
    }
}
