//! Inference smoke test. Loads a GGUF model and streams tokens to stdout via
//! the `InferenceBackend` trait (Fase 3.2).
//!
//! Usage:
//!   cargo run --release --bin llama_smoke -- \
//!     --model <path.gguf> \
//!     --prompt "<text>" \
//!     [--max-tokens N] [--n-ctx N] [--seed N] \
//!     [--gpu-layers N | --cpu]
//!
//! Lives in `src/bin/` (not `examples/` or `tests/`) for the same reason
//! `export_bindings` does — see the doc comment at the top of that file.
//!
//! By default the smoke test mirrors the app: it runs hardware detection and
//! uses GPU-first auto fallback when one is available. `--cpu` forces CPU-only;
//! `--gpu-layers N` overrides with an explicit layer count (useful to bisect
//! "is offload working at all" versus "is the chosen backend the right one").

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use abraxas_lib::hardware;
use abraxas_lib::inference::{
    GenerateParams, InferenceBackend, InferenceError, LlamaCppBackend, TokenEvent,
};
use tracing_subscriber::EnvFilter;

const USAGE: &str = "usage: llama_smoke --model <path.gguf> --prompt <text> \
                     [--max-tokens N] [--n-ctx N] [--seed N] \
                     [--gpu-layers N | --cpu]";

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with_writer(std::io::stderr)
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut model: Option<PathBuf> = None;
    let mut prompt: Option<String> = None;
    let mut max_tokens: i32 = 128;
    let mut n_ctx: u32 = 2048;
    let mut seed: u32 = 1234;
    let mut gpu_layers_override: Option<u32> = None;

    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--model" => model = it.next().map(PathBuf::from),
            "--prompt" => prompt = it.next().cloned(),
            "--max-tokens" => {
                max_tokens = match it.next().and_then(|s| s.parse().ok()) {
                    Some(n) => n,
                    None => return usage_error("--max-tokens requires an integer"),
                }
            }
            "--n-ctx" => {
                n_ctx = match it.next().and_then(|s| s.parse().ok()) {
                    Some(n) => n,
                    None => return usage_error("--n-ctx requires an integer"),
                }
            }
            "--seed" => {
                seed = match it.next().and_then(|s| s.parse().ok()) {
                    Some(n) => n,
                    None => return usage_error("--seed requires an integer"),
                }
            }
            "--gpu-layers" => {
                gpu_layers_override = match it.next().and_then(|s| s.parse().ok()) {
                    Some(n) => Some(n),
                    None => return usage_error("--gpu-layers requires an integer"),
                }
            }
            "--cpu" => gpu_layers_override = Some(0),
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            other => return usage_error(&format!("unknown argument: {other}")),
        }
    }

    let (Some(model), Some(prompt)) = (model, prompt) else {
        return usage_error("both --model and --prompt are required");
    };

    // Mirror the app's startup pipeline so the smoke test exercises the full
    // hardware → backend wiring (Fase 3.4). Skipped when `--gpu-layers` /
    // `--cpu` is set so callers can force a specific configuration.
    let use_auto_gpu = match gpu_layers_override {
        Some(n) => {
            tracing::info!(gpu_layers = n, "gpu-layers override from CLI");
            false
        }
        None => {
            let system = hardware::system::detect();
            let gpu = hardware::gpu::detect();
            let choice = hardware::selector::select_backend(&system, &gpu);
            tracing::info!(
                backend = ?choice.backend,
                reason = %choice.reason,
                "selected inference backend",
            );
            !matches!(choice.backend, hardware::selector::InferenceBackend::Cpu)
        }
    };

    // Echo the prompt so the user sees full context as completion streams in.
    print!("{prompt}");
    let _ = std::io::stdout().flush();

    let mut params = GenerateParams::new(prompt);
    params.max_tokens = max_tokens;
    params.n_ctx = n_ctx;
    params.seed = seed;

    let result: Result<(), InferenceError> = tauri::async_runtime::block_on(async move {
        let backend = match gpu_layers_override {
            Some(0) => LlamaCppBackend::new_cpu(),
            Some(n) => LlamaCppBackend::new(n),
            None if use_auto_gpu => LlamaCppBackend::new_auto_gpu(),
            None => LlamaCppBackend::new_cpu(),
        };
        backend.load_model(&model).await?;
        let mut stream = backend.generate_stream(params).await?;
        while let Some(event) = stream.recv().await {
            match event? {
                TokenEvent::Chunk(s) => {
                    print!("{s}");
                    let _ = std::io::stdout().flush();
                }
                TokenEvent::End(_) => break,
            }
        }
        Ok(())
    });
    println!();

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn usage_error(msg: &str) -> ExitCode {
    eprintln!("{msg}");
    eprintln!("{USAGE}");
    ExitCode::from(2)
}
