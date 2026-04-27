//! Fase 3.3 model lifecycle manager.
//!
//! Holds at most one model loaded at a time across the whole app. The backend
//! is referenced as `Arc<dyn InferenceBackend>` so Fase 3.4 can swap in a
//! Metal/CUDA/Vulkan-built `LlamaCppBackend` (or a different engine entirely)
//! without touching call sites.
//!
//! Two-lock design:
//!   - `loaded` (`RwLock`) — cheap concurrent reads of "what's loaded right
//!     now" for status polls and `current()`.
//!   - `lifecycle` (`Mutex`) — serializes `load`/`unload` so two concurrent
//!     loads can't run a pair of `spawn_blocking` model loads in parallel and
//!     spike RAM with two full GGUFs in memory.
//!
//! `load(path)` enforces the spec wording "descarrega anterior antes de
//! carregar novo" by calling `backend.unload()` first when swapping, instead
//! of relying on `LlamaCppBackend`'s internal load-then-swap (which briefly
//! peaks at 2x model RAM — risky for the casual-hardware persona).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use tauri::async_runtime::{Mutex, RwLock};

use crate::inference::backend::{GenerateParams, InferenceBackend, TokenStream};
use crate::inference::InferenceError;

#[derive(Debug, Clone)]
pub struct LoadedModel {
    pub path: PathBuf,
    pub loaded_at: SystemTime,
}

pub struct ModelManager {
    backend: Arc<dyn InferenceBackend>,
    loaded: RwLock<Option<LoadedModel>>,
    lifecycle: Mutex<()>,
}

impl ModelManager {
    pub fn new(backend: Arc<dyn InferenceBackend>) -> Self {
        Self {
            backend,
            loaded: RwLock::new(None),
            lifecycle: Mutex::new(()),
        }
    }

    pub async fn load(&self, path: PathBuf) -> Result<LoadedModel, InferenceError> {
        let _guard = self.lifecycle.lock().await;

        if let Some(current) = self.loaded.read().await.as_ref() {
            if current.path == path {
                return Ok(current.clone());
            }
        }

        let previous = self.loaded.write().await.take();
        if previous.is_some() {
            self.backend.unload().await?;
        }

        self.backend.load_model(&path).await?;
        let info = LoadedModel {
            path: path.clone(),
            loaded_at: SystemTime::now(),
        };
        *self.loaded.write().await = Some(info.clone());

        match &previous {
            Some(prev) => tracing::info!(
                previous = %prev.path.display(),
                next = %path.display(),
                "model swapped",
            ),
            None => tracing::info!(path = %path.display(), "model loaded"),
        }

        Ok(info)
    }

    pub async fn unload(&self) -> Result<(), InferenceError> {
        let _guard = self.lifecycle.lock().await;
        let previous = self.loaded.write().await.take();
        if let Some(prev) = previous {
            self.backend.unload().await?;
            tracing::info!(path = %prev.path.display(), "model unloaded");
        }
        Ok(())
    }

    pub async fn generate(&self, params: GenerateParams) -> Result<TokenStream, InferenceError> {
        self.backend.generate_stream(params).await
    }

    pub async fn current(&self) -> Option<LoadedModel> {
        self.loaded.read().await.clone()
    }

    /// Non-blocking status poll. If `loaded` is currently write-locked
    /// (load/unload in progress), reports `false` — the correct UX answer
    /// during a swap.
    pub fn is_loaded(&self) -> bool {
        self.loaded.try_read().map(|g| g.is_some()).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex as StdMutex;

    use async_trait::async_trait;

    use crate::inference::backend::{GenerateParams, InferenceBackend, TokenStream};

    struct MockBackend {
        load_calls: AtomicUsize,
        unload_calls: AtomicUsize,
        in_flight_loads: AtomicUsize,
        max_concurrent_loads: AtomicUsize,
        call_order: StdMutex<Vec<String>>,
    }

    impl MockBackend {
        fn new() -> Self {
            Self {
                load_calls: AtomicUsize::new(0),
                unload_calls: AtomicUsize::new(0),
                in_flight_loads: AtomicUsize::new(0),
                max_concurrent_loads: AtomicUsize::new(0),
                call_order: StdMutex::new(Vec::new()),
            }
        }

        fn record(&self, s: String) {
            self.call_order.lock().unwrap().push(s);
        }

        fn order_snapshot(&self) -> Vec<String> {
            self.call_order.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl InferenceBackend for MockBackend {
        async fn load_model(&self, path: &Path) -> Result<(), InferenceError> {
            let now = self.in_flight_loads.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_concurrent_loads.fetch_max(now, Ordering::SeqCst);
            // Yield several times so a concurrently-polled task gets a chance
            // to enter (would observe in_flight == 2 if no serialization).
            for _ in 0..32 {
                tokio::task::yield_now().await;
            }
            self.in_flight_loads.fetch_sub(1, Ordering::SeqCst);
            self.load_calls.fetch_add(1, Ordering::SeqCst);
            self.record(format!("load:{}", path.display()));
            Ok(())
        }

        async fn unload(&self) -> Result<(), InferenceError> {
            self.unload_calls.fetch_add(1, Ordering::SeqCst);
            self.record("unload".to_owned());
            Ok(())
        }

        async fn generate_stream(
            &self,
            _params: GenerateParams,
        ) -> Result<TokenStream, InferenceError> {
            Err(InferenceError::NoModelLoaded)
        }

        fn is_loaded(&self) -> bool {
            // Manager tracks its own "loaded" state; backend's flag is unused.
            false
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn load_then_load_same_path_is_idempotent() {
        let mock = Arc::new(MockBackend::new());
        let manager = ModelManager::new(mock.clone());
        let p = PathBuf::from("a");
        manager.load(p.clone()).await.unwrap();
        manager.load(p).await.unwrap();
        assert_eq!(mock.load_calls.load(Ordering::SeqCst), 1);
        assert_eq!(mock.unload_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn load_different_path_unloads_first() {
        let mock = Arc::new(MockBackend::new());
        let manager = ModelManager::new(mock.clone());
        manager.load(PathBuf::from("a")).await.unwrap();
        manager.load(PathBuf::from("b")).await.unwrap();
        assert_eq!(mock.order_snapshot(), vec!["load:a", "unload", "load:b"]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unload_when_empty_is_noop() {
        let mock = Arc::new(MockBackend::new());
        let manager = ModelManager::new(mock.clone());
        manager.unload().await.unwrap();
        assert_eq!(mock.unload_calls.load(Ordering::SeqCst), 0);
        assert!(manager.current().await.is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unload_clears_current() {
        let mock = Arc::new(MockBackend::new());
        let manager = ModelManager::new(mock);
        manager.load(PathBuf::from("a")).await.unwrap();
        assert!(manager.is_loaded());
        manager.unload().await.unwrap();
        assert!(!manager.is_loaded());
        assert!(manager.current().await.is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn generate_without_load_errors() {
        let mock = Arc::new(MockBackend::new());
        let manager = ModelManager::new(mock);
        let err = manager
            .generate(GenerateParams::new("hi"))
            .await
            .unwrap_err();
        assert!(matches!(err, InferenceError::NoModelLoaded));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn concurrent_loads_serialize() {
        let mock = Arc::new(MockBackend::new());
        let manager = ModelManager::new(mock.clone());
        let (r1, r2) = tokio::join!(
            manager.load(PathBuf::from("a")),
            manager.load(PathBuf::from("b")),
        );
        r1.unwrap();
        r2.unwrap();
        // With the lifecycle mutex, only one `load_model` runs in the backend
        // at any time → max concurrent == 1. Without it, the two yielding
        // futures would interleave and reach 2.
        assert_eq!(mock.max_concurrent_loads.load(Ordering::SeqCst), 1);
        assert_eq!(mock.load_calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn current_returns_path_and_recent_timestamp() {
        let mock = Arc::new(MockBackend::new());
        let manager = ModelManager::new(mock);
        let before = SystemTime::now();
        manager.load(PathBuf::from("a")).await.unwrap();
        let after = SystemTime::now();
        let info = manager.current().await.expect("loaded");
        assert_eq!(info.path, PathBuf::from("a"));
        assert!(info.loaded_at >= before && info.loaded_at <= after);
    }
}
