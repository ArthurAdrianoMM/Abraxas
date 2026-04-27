//! Phase 3.1 smoke test. Loads a GGUF model and streams tokens to stdout.
//!
//! Usage:
//!   cargo run --release --bin llama_smoke -- \
//!     --model <path.gguf> \
//!     --prompt "<text>" \
//!     [--max-tokens N] [--n-ctx N] [--seed N]
//!
//! Lives in `src/bin/` (not `examples/` or `tests/`) for the same reason
//! `export_bindings` does — see the doc comment at the top of that file.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use abraxas_lib::inference::llama_cpp::{generate, GenerateOpts};
use tracing_subscriber::EnvFilter;

const USAGE: &str = "usage: llama_smoke --model <path.gguf> --prompt <text> \
                     [--max-tokens N] [--n-ctx N] [--seed N]";

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

    // Echo the prompt so the user sees full context as completion streams in.
    print!("{prompt}");
    let _ = std::io::stdout().flush();

    let opts = GenerateOpts {
        model_path: &model,
        prompt: &prompt,
        max_tokens,
        n_ctx,
        seed,
    };

    let result = generate(opts, |piece| {
        print!("{piece}");
        let _ = std::io::stdout().flush();
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
