//! Inference commands for Phase 5.1.
//!
//! `start_generation` accepts a structured chat history and the per-call
//! generation options, looks up the chat template + context budget bound to
//! the currently-loaded model (set by `load_installed_model`), truncates
//! history to fit the context window, renders the prompt with the
//! family-correct chat template, and streams tokens back via
//! `GenerationEvent`s.
//!
//! The single-flight `GenerationRegistry` enforces "one generation in
//! flight at a time" — aligned with the "one model loaded at a time"
//! invariant from Fase 3.3.
//!
//! Conversation persistence (Fase 5.2) is intentionally not handled here:
//! this command is the wire between an arbitrary message list and the
//! inference engine. The DB-backed conversation flow will assemble the
//! message list from `messages` table rows and call this command — at
//! which point we'll add a `conversation_id` parameter for telemetry.

use std::sync::Arc;

use tauri::{AppHandle, State};
use tauri_specta::Event;

use crate::chat::templates::{bos_policy_for, ChatMessage};
use crate::chat::{
    fit_prompt_to_context, resolve_max_completion_tokens, resolve_sampling, ChatGenerationOptions,
    ContextError,
};
use crate::db::{conversations, Db};
use crate::error::{AppError, CommandError};
use crate::events::GenerationEvent;
use crate::inference::backend::{GenerateParams, TokenEvent};
use crate::inference::ModelManager;

/// Default reservation for the completion when the catalog provides a
/// context length but the caller didn't pin `max_completion_tokens`.
/// Shared with the settings default so the two never drift apart.
const DEFAULT_COMPLETION_BUDGET: u32 = crate::db::app_settings::DEFAULT_MAX_COMPLETION_TOKENS;

/// Hard cap on `n_ctx` used when the loaded model has no catalog-declared
/// context length (legacy / dev loads). Matches the previous `GenerateParams`
/// default and keeps memory bounded.
const FALLBACK_N_CTX: u32 = 2048;

/// Holds the single in-flight generation. Aligns with the
/// "one model loaded at a time" invariant from Fase 3.3.
#[derive(Default)]
pub struct GenerationRegistry {
    current: tauri::async_runtime::Mutex<Option<Active>>,
}

struct Active {
    id: String,
    handle: tauri::async_runtime::JoinHandle<()>,
}

#[tauri::command]
#[specta::specta]
pub async fn start_generation(
    app: AppHandle,
    manager: State<'_, Arc<ModelManager>>,
    registry: State<'_, Arc<GenerationRegistry>>,
    db: State<'_, Db>,
    messages: Vec<ChatMessage>,
    options: Option<ChatGenerationOptions>,
    conversation_id: Option<String>,
) -> Result<String, CommandError> {
    if messages.is_empty() {
        return Err(CommandError {
            kind: "Inference".into(),
            message: "cannot start generation with an empty message list".into(),
        });
    }

    let mut slot = registry.current.lock().await;
    if slot.is_some() {
        return Err(CommandError {
            kind: "Inference".into(),
            message: "another generation is already in progress".into(),
        });
    }

    // Resolve what's loaded — without a model, there's no template either.
    let loaded = manager.current().await.ok_or_else(|| CommandError {
        kind: "Inference".into(),
        message: "no model is loaded; call load_installed_model first".into(),
    })?;
    let template = loaded.chat_template.ok_or_else(|| CommandError {
        kind: "Inference".into(),
        message: "loaded model has no associated chat template; reload via load_installed_model"
            .into(),
    })?;

    let options = options.unwrap_or_default();

    // Fase 5.4: layer per-call options on top of the conversation's stored
    // generation params, falling back to defaults for any field left NULL.
    let conversation = match conversation_id.as_deref() {
        Some(id) => conversations::get(db.pool(), id)
            .await
            .map_err(AppError::Db)?,
        None => None,
    };
    if conversation_id.is_some() && conversation.is_none() {
        return Err(CommandError {
            kind: "ConversationNotFound".into(),
            message: format!(
                "conversation {:?} does not exist",
                conversation_id.as_deref().unwrap_or_default()
            ),
        });
    }

    let sampling = resolve_sampling(conversation.as_ref(), options.sampling);
    let resolved_max_completion =
        resolve_max_completion_tokens(conversation.as_ref(), options.max_completion_tokens);

    // Context budget: cap n_ctx by the catalog's declared model max so we
    // never request a window the model wasn't trained for.
    let n_ctx = loaded.context_length.unwrap_or(FALLBACK_N_CTX);
    let completion_budget = resolved_max_completion
        .map(|n| n.max(1) as u32)
        .unwrap_or(DEFAULT_COMPLETION_BUDGET)
        .min(n_ctx.saturating_sub(1));

    let bos_policy = bos_policy_for(template);
    let manager_for_count = Arc::clone(&manager);
    // Fase 5.3: trim oldest non-system messages until the rendered prompt
    // fits the model's context window, using the loaded model's tokenizer
    // for exact counts (no heuristic drift between count and generate).
    let fitted = fit_prompt_to_context(
        template,
        &messages,
        n_ctx,
        completion_budget,
        move |prompt| {
            let manager = Arc::clone(&manager_for_count);
            async move { manager.count_tokens(prompt, bos_policy).await }
        },
    )
    .await
    .map_err(context_error_to_command)?;

    // `max_tokens` in `GenerateParams` is total positions (prompt + completion).
    // We now have the exact prompt count from the loaded model tokenizer, so
    // cap the backend at prompt + requested completion, never beyond n_ctx.
    let max_tokens = fitted
        .prompt_tokens
        .saturating_add(completion_budget as usize)
        .min(n_ctx as usize) as i32;

    let params = GenerateParams {
        prompt: fitted.prompt,
        max_tokens,
        n_ctx,
        bos_policy,
        sampling,
    };

    let mut stream = manager.generate(params).await.map_err(AppError::from)?;
    let id = uuid::Uuid::new_v4().to_string();
    let _ = GenerationEvent::Started {
        generation_id: id.clone(),
    }
    .emit(&app);

    let app_h = app.clone();
    let id_h = id.clone();
    let registry_h: Arc<GenerationRegistry> = Arc::clone(&registry);
    let completion_cap = completion_budget as usize;
    let handle = tauri::async_runtime::spawn(async move {
        // Track the number of tokens we've emitted so we honor the
        // user-provided completion budget. The backend's `max_tokens`
        // is a coarse total-position ceiling, not a completion-only one.
        let mut emitted_tokens: usize = 0;
        loop {
            match stream.recv().await {
                Some(Ok(TokenEvent::Chunk(text))) => {
                    let _ = GenerationEvent::Token {
                        generation_id: id_h.clone(),
                        text,
                    }
                    .emit(&app_h);
                    emitted_tokens += 1;
                    if emitted_tokens >= completion_cap {
                        let _ = GenerationEvent::End {
                            generation_id: id_h.clone(),
                            reason: crate::inference::backend::StopReason::MaxTokens.into(),
                        }
                        .emit(&app_h);
                        // Drop the stream; the backend exits on closed channel.
                        drop(stream);
                        break;
                    }
                }
                Some(Ok(TokenEvent::End(stop))) => {
                    let _ = GenerationEvent::End {
                        generation_id: id_h.clone(),
                        reason: stop.into(),
                    }
                    .emit(&app_h);
                    break;
                }
                Some(Err(e)) => {
                    let _ = GenerationEvent::Failed {
                        generation_id: id_h.clone(),
                        kind: "Inference".into(),
                        message: e.to_string(),
                    }
                    .emit(&app_h);
                    break;
                }
                None => break,
            }
        }
        // Clear the slot if we still own it. `cancel_generation` may have
        // already taken it; in that case we leave it untouched.
        let mut slot = registry_h.current.lock().await;
        if slot.as_ref().map(|a| a.id == id_h).unwrap_or(false) {
            *slot = None;
        }
    });

    *slot = Some(Active {
        id: id.clone(),
        handle,
    });
    Ok(id)
}

fn context_error_to_command(e: ContextError) -> CommandError {
    let kind = match &e {
        ContextError::Template(_) => "Template",
        ContextError::TokenCount(_) | ContextError::PromptTooLarge { .. } => "Context",
    };
    CommandError {
        kind: kind.into(),
        message: e.to_string(),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_generation(
    app: AppHandle,
    registry: State<'_, Arc<GenerationRegistry>>,
    generation_id: String,
) -> Result<(), CommandError> {
    let mut slot = registry.current.lock().await;
    if slot
        .as_ref()
        .map(|a| a.id == generation_id)
        .unwrap_or(false)
    {
        let active = slot.take().expect("just checked");
        let _ = GenerationEvent::Cancelled {
            generation_id: active.id,
        }
        .emit(&app);
        active.handle.abort();
    }
    Ok(())
}
