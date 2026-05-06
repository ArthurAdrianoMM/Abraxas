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

use crate::chat::templates::BosPolicy;
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
    gpu_layers: u32,
}

const FULL_GPU_OFFLOAD_LAYERS: u32 = 999;
const AUTO_GPU_FALLBACK_LAYERS: [u32; 5] = [FULL_GPU_OFFLOAD_LAYERS, 32, 16, 8, 0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffloadPolicy {
    CpuOnly,
    FixedGpuLayers(u32),
    AutoGpuFallback,
}

impl OffloadPolicy {
    fn attempts(self) -> Vec<u32> {
        match self {
            OffloadPolicy::CpuOnly => vec![0],
            OffloadPolicy::FixedGpuLayers(n) => vec![n],
            OffloadPolicy::AutoGpuFallback => AUTO_GPU_FALLBACK_LAYERS.to_vec(),
        }
    }
}

pub struct LlamaCppBackend {
    state: Arc<RwLock<Option<LoadedModel>>>,
    offload_policy: OffloadPolicy,
}

impl LlamaCppBackend {
    /// Fixed-layer constructor kept for smoke tests and callers that need an
    /// exact llama.cpp `n_gpu_layers` value. `0` means CPU-only.
    pub fn new(gpu_layers: u32) -> Self {
        Self::with_offload_policy(OffloadPolicy::FixedGpuLayers(gpu_layers))
    }

    pub fn with_offload_policy(offload_policy: OffloadPolicy) -> Self {
        Self {
            state: Arc::new(RwLock::new(None)),
            offload_policy,
        }
    }

    /// CPU-only constructor. Used by tests and by `lib.rs` when
    /// `hardware::selector` returns `InferenceBackend::Cpu`.
    pub fn new_cpu() -> Self {
        Self::with_offload_policy(OffloadPolicy::CpuOnly)
    }

    /// GPU-first constructor for normal app use. It tries full offload first,
    /// then progressively smaller partial offloads, then CPU-only fallback.
    pub fn new_auto_gpu() -> Self {
        Self::with_offload_policy(OffloadPolicy::AutoGpuFallback)
    }
}

impl Default for LlamaCppBackend {
    fn default() -> Self {
        Self::new_cpu()
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

        let mut loaded_model = None;
        let mut last_error = None;
        for gpu_layers in self.offload_policy.attempts() {
            let load_path = path.clone();
            let backend = backend.clone();
            let result = async_runtime::spawn_blocking(move || {
                load_model_blocking(&backend, &load_path, gpu_layers)
            })
            .await
            .expect("model-load task panicked");

            match result {
                Ok(model) => {
                    tracing::info!(
                        path = %path.display(),
                        gpu_layers,
                        "model load offload attempt succeeded",
                    );
                    loaded_model = Some((model, gpu_layers));
                    break;
                }
                Err(e) => {
                    tracing::warn!(
                        path = %path.display(),
                        gpu_layers,
                        error = %e,
                        "model load offload attempt failed",
                    );
                    last_error = Some(e);
                }
            }
        }

        let Some((model, gpu_layers)) = loaded_model else {
            return Err(last_error.expect("offload policy must produce at least one attempt"));
        };

        let loaded = LoadedModel {
            path: path.clone(),
            model,
            gpu_layers,
        };
        let previous = {
            let mut guard = self.state.write().await;
            guard.replace(loaded)
        };
        match previous {
            Some(prev) => tracing::info!(
                previous = %prev.path.display(),
                next = %path.display(),
                previous_gpu_layers = prev.gpu_layers,
                gpu_layers,
                "swapped loaded model",
            ),
            None => tracing::info!(path = %path.display(), gpu_layers, "model loaded"),
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

    async fn generate_stream(&self, params: GenerateParams) -> Result<TokenStream, InferenceError> {
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

    async fn count_tokens(
        &self,
        prompt: String,
        bos_policy: BosPolicy,
    ) -> Result<usize, InferenceError> {
        let model = {
            let guard = self.state.read().await;
            guard
                .as_ref()
                .ok_or(InferenceError::NoModelLoaded)?
                .model
                .clone()
        };

        async_runtime::spawn_blocking(move || {
            let add_bos = add_bos_for(bos_policy);
            model
                .str_to_token(&prompt, add_bos)
                .map(|tokens| tokens.len())
                .map_err(InferenceError::from)
        })
        .await
        .expect("token-count task panicked")
    }

    fn is_loaded(&self) -> bool {
        // try_read so a status poll never blocks. "Currently loading" reads as
        // not-yet-loaded, which is the correct UX answer.
        self.state.try_read().map(|g| g.is_some()).unwrap_or(false)
    }
}

fn load_model_blocking(
    backend: &LlamaBackend,
    path: &Path,
    gpu_layers: u32,
) -> Result<Arc<LlamaModel>, InferenceError> {
    // With both CUDA and Vulkan compiled in (Windows/Linux), llama.cpp's ggml
    // registry picks devices by registration order. Precise CUDA-vs-Vulkan
    // filtering via `with_devices` remains deferred until llama-cpp-2 exposes a
    // stable device-enumeration API.
    let params = LlamaModelParams::default().with_n_gpu_layers(gpu_layers);
    let model = LlamaModel::load_from_file(backend, path, &params)?;
    Ok(Arc::new(model))
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

    // Templates that emit a literal BOS marker (Llama3 `<|begin_of_text|>`,
    // Llama2/Mistral `<s>`, DeepSeek/GLM4 equivalents) MUST tokenize without
    // the tokenizer re-adding BOS — otherwise the model sees two BOS tokens
    // and quality degrades silently. See `chat::templates::bos_policy_for`.
    let add_bos = add_bos_for(params.bos_policy);
    let tokens = model.str_to_token(&params.prompt, add_bos)?;
    let prompt_len = tokens.len();

    let mut batch = LlamaBatch::new(512, 1);
    let last_idx = (prompt_len - 1) as i32;
    for (i, token) in (0_i32..).zip(tokens) {
        batch.add(token, i, &[0], i == last_idx)?;
    }
    ctx.decode(&mut batch)?;

    let mut sampler = build_sampler(&params.sampling);
    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let mut n_cur = batch.n_tokens();
    let mut stop = StopReason::MaxTokens;

    // `max_tokens` is the total position budget (prompt + completion). Stop
    // before emitting a token that would push position count past the budget.
    while n_cur < params.max_tokens {
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

fn add_bos_for(policy: BosPolicy) -> AddBos {
    match policy {
        BosPolicy::Always => AddBos::Always,
        BosPolicy::Never => AddBos::Never,
    }
}

/// Build a sampler chain matching llama.cpp's standard pipeline:
///   penalties → top-k → top-p → temperature → dist (final picker).
/// `temperature == 0` collapses the chain to repetition-aware greedy.
fn build_sampler(s: &crate::chat::SamplingParams) -> LlamaSampler {
    let mut stages: Vec<LlamaSampler> = Vec::new();

    if s.repeat_penalty > 1.0 && s.repeat_last_n != 0 {
        stages.push(LlamaSampler::penalties(
            s.repeat_last_n,
            s.repeat_penalty,
            0.0, // frequency_penalty
            0.0, // presence_penalty
        ));
    }

    if s.temperature <= 0.0 {
        stages.push(LlamaSampler::greedy());
        return LlamaSampler::chain_simple(stages);
    }

    if s.top_k > 0 {
        stages.push(LlamaSampler::top_k(s.top_k));
    }
    if s.top_p > 0.0 && s.top_p < 1.0 {
        stages.push(LlamaSampler::top_p(s.top_p, 1));
    }
    stages.push(LlamaSampler::temp(s.temperature));
    stages.push(LlamaSampler::dist(s.seed));
    LlamaSampler::chain_simple(stages)
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
        let backend = LlamaCppBackend::new_cpu();
        let err = backend
            .generate_stream(GenerateParams::new("hi"))
            .await
            .unwrap_err();
        assert!(matches!(err, InferenceError::NoModelLoaded));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn count_tokens_without_loaded_model_errors() {
        let backend = LlamaCppBackend::new_cpu();
        let err = backend
            .count_tokens("hi".to_owned(), BosPolicy::Always)
            .await
            .unwrap_err();
        assert!(matches!(err, InferenceError::NoModelLoaded));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unload_without_loaded_model_is_ok() {
        LlamaCppBackend::new_cpu().unload().await.unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn is_loaded_starts_false() {
        assert!(!LlamaCppBackend::new_cpu().is_loaded());
    }

    #[test]
    fn cpu_policy_attempts_cpu_only() {
        assert_eq!(OffloadPolicy::CpuOnly.attempts(), vec![0]);
    }

    #[test]
    fn fixed_policy_attempts_exact_layer_count() {
        assert_eq!(OffloadPolicy::FixedGpuLayers(42).attempts(), vec![42]);
    }

    #[test]
    fn auto_gpu_policy_attempts_full_partial_then_cpu() {
        assert_eq!(
            OffloadPolicy::AutoGpuFallback.attempts(),
            vec![999, 32, 16, 8, 0],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn load_missing_path_errors_before_initializing_backend() {
        let backend = LlamaCppBackend::new_cpu();
        let err = backend
            .load_model(Path::new("/definitely/does/not/exist.gguf"))
            .await
            .unwrap_err();
        assert!(matches!(err, InferenceError::ModelNotFound(_)));
    }
}
