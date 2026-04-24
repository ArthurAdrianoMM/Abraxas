//! Structured logging: JSON to rolling file + pretty console (dev) + panic capture.

use std::sync::Mutex;

use tauri::{AppHandle, Manager};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::error::AppError;

const DEFAULT_FILTER: &str = "abraxas_lib=info,abraxas=info,warn";

/// Held in Tauri state for the app's lifetime so the non-blocking
/// appender flushes on shutdown.
pub struct LogGuard(#[allow(dead_code)] Mutex<WorkerGuard>);

pub fn init(app: &AppHandle) -> Result<LogGuard, AppError> {
    let dir = app.path().app_log_dir()?;
    std::fs::create_dir_all(&dir)?;

    let file_appender = tracing_appender::rolling::daily(&dir, "abraxas.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_env("ABRAXAS_LOG")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));

    let file_layer = fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(false)
        .with_writer(file_writer);

    let console_layer = cfg!(debug_assertions).then(|| {
        fmt::layer()
            .compact()
            .with_ansi(true)
            .with_target(true)
    });

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(console_layer)
        .try_init()?;

    install_panic_hook();

    Ok(LogGuard(Mutex::new(guard)))
}

fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<non-string panic payload>".into());
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".into());
        tracing::error!(panic.payload = %payload, panic.location = %location, "panic");
        previous(info);
    }));
}
