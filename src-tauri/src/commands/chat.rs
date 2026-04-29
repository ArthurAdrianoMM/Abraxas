//! Inference-related commands (Fase 3.5).
//!
//! Surfaces `start_generation` / `cancel_generation` to the frontend, plus a
//! transient `dev_load_model` used by the temporary 3.5 dev panel. The dev
//! command is replaced by Fase 4's catalog-driven load flow.

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, State};
use tauri_specta::Event;

use crate::error::{AppError, CommandError};
use crate::events::GenerationEvent;
use crate::inference::backend::{GenerateParams, TokenEvent};
use crate::inference::ModelManager;

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

/// **TEMPORARY**: removed in Fase 4 once the catalog/download flow ships.
#[tauri::command]
#[specta::specta]
pub async fn dev_load_model(
    manager: State<'_, Arc<ModelManager>>,
    path: String,
) -> Result<(), CommandError> {
    manager
        .load(PathBuf::from(path))
        .await
        .map_err(AppError::from)?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn start_generation(
    app: AppHandle,
    manager: State<'_, Arc<ModelManager>>,
    registry: State<'_, Arc<GenerationRegistry>>,
    prompt: String,
    max_tokens: Option<i32>,
) -> Result<String, CommandError> {
    let mut slot = registry.current.lock().await;
    if slot.is_some() {
        return Err(CommandError {
            kind: "Inference".into(),
            message: "another generation is already in progress".into(),
        });
    }

    let mut params = GenerateParams::new(prompt);
    if let Some(n) = max_tokens {
        params.max_tokens = n;
    }

    let mut stream = manager.generate(params).await.map_err(AppError::from)?;
    let id = uuid::Uuid::new_v4().to_string();
    let _ = GenerationEvent::Started {
        generation_id: id.clone(),
    }
    .emit(&app);

    let app_h = app.clone();
    let id_h = id.clone();
    let registry_h: Arc<GenerationRegistry> = Arc::clone(&registry);
    let handle = tauri::async_runtime::spawn(async move {
        loop {
            match stream.recv().await {
                Some(Ok(TokenEvent::Chunk(text))) => {
                    let _ = GenerationEvent::Token {
                        generation_id: id_h.clone(),
                        text,
                    }
                    .emit(&app_h);
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
