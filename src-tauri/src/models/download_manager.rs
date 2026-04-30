//! Single-flight download slot (Fase 4.3).
//!
//! Only one model download is allowed at a time, matching the project's
//! "one model at a time" ethos (CLAUDE.md §3.3) and keeping bandwidth +
//! disk pressure predictable on a casual user's machine. A second
//! `start_model_download` invocation while another is active returns
//! `AlreadyInProgress` so the UI can surface a clear error.

use std::sync::Mutex;

use crate::models::download::CancelFlag;

#[derive(Debug)]
struct ActiveDownload {
    model_id: String,
    cancel: CancelFlag,
}

#[derive(Default)]
pub struct DownloadManager {
    inner: Mutex<Option<ActiveDownload>>,
}

impl DownloadManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reserve the slot for `model_id`. Returns the cancel flag the caller
    /// should pass to `download_model`. Returns `None` if a download is
    /// already in progress.
    pub fn start(&self, model_id: &str) -> Option<CancelFlag> {
        let mut slot = self.inner.lock().expect("download slot poisoned");
        if slot.is_some() {
            return None;
        }
        let cancel = CancelFlag::new();
        *slot = Some(ActiveDownload {
            model_id: model_id.to_owned(),
            cancel: cancel.clone(),
        });
        Some(cancel)
    }

    /// Release the slot if it currently belongs to `model_id`.
    pub fn finish(&self, model_id: &str) {
        let mut slot = self.inner.lock().expect("download slot poisoned");
        if slot
            .as_ref()
            .map(|a| a.model_id == model_id)
            .unwrap_or(false)
        {
            *slot = None;
        }
    }

    /// Fire the cancel flag for `model_id` if it matches the active slot.
    /// The download task itself clears the slot on exit via `finish`.
    pub fn cancel(&self, model_id: &str) {
        let slot = self.inner.lock().expect("download slot poisoned");
        if let Some(a) = slot.as_ref() {
            if a.model_id == model_id {
                a.cancel.cancel();
            }
        }
    }

    pub fn active_id(&self) -> Option<String> {
        self.inner
            .lock()
            .expect("download slot poisoned")
            .as_ref()
            .map(|a| a.model_id.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_start_is_rejected() {
        let m = DownloadManager::new();
        assert!(m.start("a").is_some());
        assert!(m.start("b").is_none());
        assert_eq!(m.active_id().as_deref(), Some("a"));
    }

    #[test]
    fn finish_releases_slot() {
        let m = DownloadManager::new();
        m.start("a").unwrap();
        m.finish("a");
        assert!(m.start("b").is_some());
    }

    #[test]
    fn finish_with_wrong_id_is_noop() {
        let m = DownloadManager::new();
        m.start("a").unwrap();
        m.finish("other");
        assert_eq!(m.active_id().as_deref(), Some("a"));
    }

    #[test]
    fn cancel_fires_flag_only_for_active_id() {
        let m = DownloadManager::new();
        let flag = m.start("a").unwrap();
        m.cancel("other");
        assert!(!flag.is_cancelled());
        m.cancel("a");
        assert!(flag.is_cancelled());
    }
}
