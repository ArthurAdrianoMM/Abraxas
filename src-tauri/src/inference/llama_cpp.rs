//! Phase 3.1 inference path: load a GGUF model from disk and stream tokens
//! via a caller-supplied callback. Synchronous, single-threaded, CPU-only.
//!
//! The trait abstraction (`InferenceBackend`) lands in 3.2, the lifecycle
//! manager in 3.3, cross-platform Cargo features (Metal/CUDA/Vulkan) in 3.4,
//! and the Tauri-event streaming wiring in 3.5. Anything beyond "load model,
//! produce tokens, return" is intentionally absent here.
//!
//! The generation loop mirrors `examples/simple/src/main.rs` from
//! https://github.com/utilityai/llama-cpp-rs — the upstream is the source of
//! truth for token-by-token decoding with `encoding_rs` UTF-8 stitching.

use std::num::NonZeroU32;
use std::path::Path;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;

use crate::inference::InferenceError;

pub struct GenerateOpts<'a> {
    pub model_path: &'a Path,
    pub prompt: &'a str,
    /// Total positions (prompt + completion) — matches the upstream
    /// example's `n_len` semantics.
    pub max_tokens: i32,
    pub n_ctx: u32,
    pub seed: u32,
}

pub fn generate<F>(opts: GenerateOpts<'_>, mut on_token: F) -> Result<(), InferenceError>
where
    F: FnMut(&str),
{
    if !opts.model_path.exists() {
        return Err(InferenceError::ModelNotFound(opts.model_path.to_path_buf()));
    }

    let backend = LlamaBackend::init()?;

    let model_params = LlamaModelParams::default();
    let model = LlamaModel::load_from_file(&backend, opts.model_path, &model_params)?;

    let ctx_params = LlamaContextParams::default().with_n_ctx(NonZeroU32::new(opts.n_ctx));
    let mut ctx = model.new_context(&backend, ctx_params)?;

    let tokens = model.str_to_token(opts.prompt, AddBos::Always)?;
    let prompt_len = tokens.len();

    // Prime the context with the prompt. Only the last token requests logits
    // — that's what the sampler reads to pick the first generated token.
    let mut batch = LlamaBatch::new(512, 1);
    let last_idx = (prompt_len - 1) as i32;
    for (i, token) in (0_i32..).zip(tokens) {
        batch.add(token, i, &[0], i == last_idx)?;
    }
    ctx.decode(&mut batch)?;

    // Greedy + dist sampler chain: dist applies the seed for reproducibility,
    // greedy picks the argmax. Phase 5 will introduce temperature/top_p.
    let mut sampler =
        LlamaSampler::chain_simple([LlamaSampler::dist(opts.seed), LlamaSampler::greedy()]);

    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let mut n_cur = batch.n_tokens();

    while n_cur <= opts.max_tokens {
        let token = sampler.sample(&ctx, batch.n_tokens() - 1);
        sampler.accept(token);

        if model.is_eog_token(token) {
            break;
        }

        let piece = model.token_to_piece(token, &mut decoder, true, None)?;
        on_token(&piece);

        batch.clear();
        batch.add(token, n_cur, &[0], true)?;
        n_cur += 1;
        ctx.decode(&mut batch)?;
    }

    Ok(())
}
