//! Typed events emitted from Rust to the frontend (Fase 3.5).
//!
//! Single tagged enum keyed by `generation_id` so future Fase 5 conversations
//! can multiplex over one event channel without changing payload shape.

use serde::Serialize;
use specta::Type;
use tauri_specta::Event;

use crate::inference::backend::StopReason;

#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GenerationEvent {
    Started {
        generation_id: String,
    },
    Token {
        generation_id: String,
        text: String,
    },
    End {
        generation_id: String,
        reason: StopReasonDto,
    },
    Failed {
        generation_id: String,
        kind: String,
        message: String,
    },
    Cancelled {
        generation_id: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum StopReasonDto {
    Eog,
    MaxTokens,
}

impl From<StopReason> for StopReasonDto {
    fn from(r: StopReason) -> Self {
        match r {
            StopReason::Eog => Self::Eog,
            StopReason::MaxTokens => Self::MaxTokens,
        }
    }
}

/// Model-download progress (Fase 4.3). Keyed by `model_id` so multiple
/// future concurrent downloads (post-MVP) don't need a separate channel.
#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DownloadEvent {
    Started {
        model_id: String,
        total_bytes: u64,
    },
    Progress {
        model_id: String,
        downloaded_bytes: u64,
        total_bytes: u64,
    },
    Completed {
        model_id: String,
        final_path: String,
    },
    Failed {
        model_id: String,
        kind: String,
        message: String,
    },
    Cancelled {
        model_id: String,
    },
}
