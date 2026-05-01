//! Context-window truncation (Fase 5.3).
//!
//! When a conversation grows past the model's context window, we drop the
//! oldest user/assistant turns until the rendered prompt fits, while always
//! preserving every system message. System messages carry the persona /
//! safety instructions; losing them silently changes model behavior in ways
//! the user can't see.
//!
//! Token counts are estimated, not exact. We don't have a tokenizer at this
//! layer (templates are family-coarse, not tokenizer-specific), so we use a
//! conservative chars/3 heuristic — empirically tighter than the common
//! chars/4 rule for English-only chat — plus per-message overhead for the
//! template's role markers. The heuristic is intentionally pessimistic: if
//! we over-estimate, we truncate slightly more than necessary; if we
//! under-estimate, the real prompt overflows `n_ctx` and llama.cpp errors
//! out — which is the worse failure mode.

use crate::chat::templates::{ChatMessage, ChatRole};

/// Reserve this many tokens of headroom on top of the completion budget so
/// a slightly-pessimistic estimate doesn't push the real prompt over.
const SAFETY_MARGIN_TOKENS: u32 = 32;

/// Per-message fixed overhead from template role markers (e.g. `<|im_start|>`
/// pairs, `[INST]` brackets). Family-independent upper bound.
const PER_MESSAGE_OVERHEAD: u32 = 8;

/// Truncate `messages` so the rendered prompt is expected to fit in
/// `n_ctx - completion_budget - SAFETY_MARGIN_TOKENS` tokens.
///
/// Always keeps:
///   - every `System` message (regardless of position),
///   - the *last* message — without it there's nothing to generate from.
///
/// Drops the oldest non-system messages first. Returns the surviving
/// messages in their original order. If even the system messages plus the
/// last message don't fit, returns them anyway: the backend will surface a
/// clearer error than we can synthesize here.
pub fn truncate_to_fit(
    messages: &[ChatMessage],
    n_ctx: u32,
    completion_budget: u32,
) -> Vec<ChatMessage> {
    if messages.is_empty() {
        return Vec::new();
    }

    let budget = n_ctx
        .saturating_sub(completion_budget)
        .saturating_sub(SAFETY_MARGIN_TOKENS);

    // Index-based to preserve original order on output.
    let total_estimate: u32 = messages.iter().map(estimate_tokens).sum();
    if total_estimate <= budget {
        return messages.to_vec();
    }

    let last_idx = messages.len() - 1;
    let mut keep = vec![false; messages.len()];

    // Pin every system message and the last message.
    for (i, m) in messages.iter().enumerate() {
        if m.role == ChatRole::System || i == last_idx {
            keep[i] = true;
        }
    }

    let mut used: u32 = messages
        .iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, m)| estimate_tokens(m))
        .sum();

    // Add older non-system, non-last messages newest-first until we'd exceed.
    for i in (0..last_idx).rev() {
        if keep[i] {
            continue;
        }
        let cost = estimate_tokens(&messages[i]);
        if used.saturating_add(cost) > budget {
            continue;
        }
        keep[i] = true;
        used += cost;
    }

    messages
        .iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, m)| m.clone())
        .collect()
}

fn estimate_tokens(message: &ChatMessage) -> u32 {
    // chars/3 is conservative for English; bumps up for tokenizer overhead
    // on punctuation and special characters in code blocks.
    let chars = message.content.chars().count() as u32;
    chars.div_ceil(3) + PER_MESSAGE_OVERHEAD
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: ChatRole, content: &str) -> ChatMessage {
        ChatMessage::new(role, content)
    }

    #[test]
    fn keeps_all_when_within_budget() {
        let msgs = vec![
            msg(ChatRole::System, "be terse"),
            msg(ChatRole::User, "hi"),
            msg(ChatRole::Assistant, "hello"),
            msg(ChatRole::User, "bye"),
        ];
        let kept = truncate_to_fit(&msgs, 4096, 256);
        assert_eq!(kept, msgs);
    }

    #[test]
    fn drops_oldest_non_system_first() {
        let msgs = vec![
            msg(ChatRole::System, "sys"),
            msg(ChatRole::User, &"a".repeat(300)),
            msg(ChatRole::Assistant, &"b".repeat(300)),
            msg(ChatRole::User, &"c".repeat(300)),
            msg(ChatRole::Assistant, &"d".repeat(300)),
            msg(ChatRole::User, "current"),
        ];
        // Tight budget: only system + last + maybe one more turn fit.
        let kept = truncate_to_fit(&msgs, 256, 64);
        assert!(kept.first().unwrap().role == ChatRole::System);
        assert_eq!(kept.last().unwrap().content, "current");
        // The very oldest user (300 a's) should be dropped before newer ones.
        assert!(!kept.iter().any(|m| m.content == "a".repeat(300)));
    }

    #[test]
    fn always_keeps_system_even_under_extreme_pressure() {
        let msgs = vec![
            msg(ChatRole::System, &"sys".repeat(100)),
            msg(ChatRole::User, &"u".repeat(1000)),
            msg(ChatRole::Assistant, &"a".repeat(1000)),
            msg(ChatRole::User, "tail"),
        ];
        let kept = truncate_to_fit(&msgs, 64, 32);
        assert!(kept.iter().any(|m| m.role == ChatRole::System));
        assert_eq!(kept.last().unwrap().content, "tail");
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(truncate_to_fit(&[], 4096, 256).is_empty());
    }
}
