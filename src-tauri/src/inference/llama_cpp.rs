//! Fase 3.2 concrete `InferenceBackend` impl for llama-cpp-2.
//!
//! Builds on Fase 3.1's decoding loop. Key differences:
//!   - `LlamaBackend::init()` is cached in a `OnceLock` (single-shot per
//!     process — upstream guards with an `AtomicBool`).
//!   - Loaded model lives in `Arc<RwLock<Option<LoadedModel>>>` so `&self`
//!     methods can swap it.
//!   - Decoding runs in `spawn_blocking`; tokens go through a bounded mpsc.
//!     Dropping the `TokenStream` causes `blocking_send` to fail → loop exits.

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use tauri::async_runtime::{self, RwLock};

use crate::inference::backend::{
    GenerateParams, InferenceBackend, StopReason, TokenEvent, TokenStream,
};
use crate::inference::InferenceError;

// `LlamaBackend::init()` is single-shot per process; calling it twice yields
// `BackendAlreadyInitialized`. `OnceLock::get_or_init` takes an infallible
// closure, so we double-check under a mutex to handle the fallible-init race.
static BACKEND: OnceLock<Arc<LlamaBackend>> = OnceLock::new();
static BACKEND_LOCK: Mutex<()> = Mutex::new(());

fn ensure_backend() -> Result<Arc<LlamaBackend>, InferenceError> {
    if let Some(b) = BACKEND.get() {
        return Ok(b.clone());
    }
    let _g = BACKEND_LOCK.lock().expect("BACKEND_LOCK poisoned");
    if let Some(b) = BACKEND.get() {
        return Ok(b.clone());
    }
    let initialized = Arc::new(LlamaBackend::init()?);
    Ok(BACKEND.get_or_init(|| initialized).clone())
}

struct LoadedModel {
    path: PathBuf,
    model: Arc<LlamaModel>,
}

pub struct LlamaCppBackend {
    state: Arc<RwLock<Option<LoadedModel>>>,
}

impl LlamaCppBackend {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(None)),
        }
    }
}

impl Default for LlamaCppBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InferenceBackend for LlamaCppBackend {
    async fn load_model(&self, path: &Path) -> Result<(), InferenceError> {
        if !path.exists() {
            return Err(InferenceError::ModelNotFound(path.to_path_buf()));
        }
        let path = path.to_path_buf();
        let backend = ensure_backend()?;

        let load_path = path.clone();
        let model = async_runtime::spawn_blocking(
            move || -> Result<Arc<LlamaModel>, InferenceError> {
                let params = LlamaModelParams::default();
                let model = LlamaModel::load_from_file(&backend, &load_path, &params)?;
                Ok(Arc::new(model))
            },
        )
        .await
        .expect("model-load task panicked")?;

        let loaded = LoadedModel {
            path: path.clone(),
            model,
        };
        let previous = {
            let mut guard = self.state.write().await;
            guard.replace(loaded)
        };
        match previous {
            Some(prev) => tracing::info!(
                previous = %prev.path.display(),
                next = %path.display(),
                "swapped loaded model",
            ),
            None => tracing::info!(path = %path.display(), "model loaded"),
        }
        Ok(())
    }

    async fn unload(&self) -> Result<(), InferenceError> {
        let mut guard = self.state.write().await;
        if let Some(prev) = guard.take() {
            tracing::info!(path = %prev.path.display(), "model unloaded");
        }
        Ok(())
    }

    async fn generate_stream(
        &self,
        params: GenerateParams,
    ) -> Result<TokenStream, InferenceError> {
        let model = {
            let guard = self.state.read().await;
            guard
                .as_ref()
                .ok_or(InferenceError::NoModelLoaded)?
                .model
                .clone()
        };
        let backend = ensure_backend()?;

        let (tx, rx) = async_runtime::channel::<Result<TokenEvent, InferenceError>>(64);
        async_runtime::spawn_blocking(move || {
            let result = generate_blocking(&backend, &model, &params, |event| {
                tx.blocking_send(Ok(event)).map_err(|_| ChannelClosed)
            });
            if let Err(e) = result {
                let _ = tx.blocking_send(Err(e));
            }
        });

        Ok(TokenStream::new(rx))
    }

    fn is_loaded(&self) -> bool {
        // try_read so a status poll never blocks. "Currently loading" reads as
        // not-yet-loaded, which is the correct UX answer.
        self.state
            .try_read()
            .map(|g| g.is_some())
            .unwrap_or(false)
    }
}

struct ChannelClosed;

fn generate_blocking<F>(
    backend: &LlamaBackend,
    model: &LlamaModel,
    params: &GenerateParams,
    mut emit: F,
) -> Result<(), InferenceError>
where
    F: FnMut(TokenEvent) -> Result<(), ChannelClosed>,
{
    let ctx_params = LlamaContextParams::default().with_n_ctx(NonZeroU32::new(params.n_ctx));
    let mut ctx = model.new_context(backend, ctx_params)?;

    let tokens = model.str_to_token(&params.prompt, AddBos::Always)?;
    let prompt_len = tokens.len();

    let mut batch = LlamaBatch::new(512, 1);
    let last_idx = (prompt_len - 1) as i32;
    for (i, token) in (0_i32..).zip(tokens) {
        batch.add(token, i, &[0], i == last_idx)?;
    }
    ctx.decode(&mut batch)?;

    let mut sampler =
        LlamaSampler::chain_simple([LlamaSampler::dist(params.seed), LlamaSampler::greedy()]);
    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let mut n_cur = batch.n_tokens();
    let mut stop = StopReason::MaxTokens;

    while n_cur <= params.max_tokens {
        let token = sampler.sample(&ctx, batch.n_tokens() - 1);
        sampler.accept(token);

        if model.is_eog_token(token) {
            stop = StopReason::Eog;
            break;
        }

        let piece = model.token_to_piece(token, &mut decoder, true, None)?;
        if emit(TokenEvent::Chunk(piece)).is_err() {
            // Consumer dropped — quiet exit, no End event.
            return Ok(());
        }

        batch.clear();
        batch.add(token, n_cur, &[0], true)?;
        n_cur += 1;
        ctx.decode(&mut batch)?;
    }

    let _ = emit(TokenEvent::End(stop));
    Ok(())
}

#[cfg(test)]
mod tests {
    // These tests deliberately avoid invoking `ensure_backend()` so they don't
    // call `LlamaBackend::init()`. Once any test in the process initializes the
    // backend, every later test that does so would error with
    // `BackendAlreadyInitialized`. Path-checks happen before init in `load_model`.
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn generate_without_loaded_model_errors() {
        let backend = LlamaCppBackend::new();
        let err = backend
            .generate_stream(GenerateParams::new("hi"))
            .await
            .unwrap_err();
        assert!(matches!(err, InferenceError::NoModelLoaded));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unload_without_loaded_model_is_ok() {
        LlamaCppBackend::new().unload().await.unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn is_loaded_starts_false() {
        assert!(!LlamaCppBackend::new().is_loaded());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn load_missing_path_errors_before_initializing_backend() {
        let backend = LlamaCppBackend::new();
        let err = backend
            .load_model(Path::new("/definitely/does/not/exist.gguf"))
            .await
            .unwrap_err();
        assert!(matches!(err, InferenceError::ModelNotFound(_)));
    }
}
