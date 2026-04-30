//! Resumable model download with SHA256 integrity verification (Fase 4.3/4.4).
//!
//! Streams a GGUF from the catalog `url` to `<models_dir>/<filename>.part`,
//! resuming via HTTP `Range` requests when a partial file already exists.
//! After streaming completes, the `.part` file is SHA256-hashed against the
//! catalog checksum; on match it is renamed to `<filename>`, on mismatch it
//! is deleted and `DownloadError::ChecksumMismatch` is returned.
//!
//! Only a file at `<filename>` (without `.part`) has passed integrity; callers
//! (registry, inference loader) can trust it without re-verifying.

use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;

use crate::models::catalog::ModelEntry;

const PROGRESS_INTERVAL: Duration = Duration::from_millis(200);
const PROGRESS_BYTES: u64 = 1 << 20; // 1 MiB

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("download HTTP error: {0}")]
    Http(String),
    #[error("server returned status {0}")]
    BadStatus(u16),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("download cancelled")]
    Cancelled,
    #[error("downloaded {got} bytes but catalog declares {expected}")]
    SizeMismatch { expected: u64, got: u64 },
    #[error("checksum mismatch: expected {expected}, got {got}")]
    ChecksumMismatch { expected: String, got: String },
}

impl From<reqwest::Error> for DownloadError {
    fn from(e: reqwest::Error) -> Self {
        DownloadError::Http(e.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct DownloadOutcome {
    pub final_path: PathBuf,
    pub bytes_written: u64,
}

/// Lightweight cancellation flag. Atomic instead of `tokio_util::CancellationToken`
/// to avoid pulling in an extra crate for one bool.
#[derive(Debug, Default, Clone)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Hash `path` with SHA256 in a blocking thread, comparing against
/// `expected_hex` (case-insensitive). `on_progress(hashed, total)` fires at
/// most every `PROGRESS_INTERVAL` or every `PROGRESS_BYTES` read. Honors
/// `cancel` between buffer reads; returns `Cancelled` without deleting the
/// file (caller decides).
async fn verify_sha256(
    path: PathBuf,
    expected_hex: &str,
    total: u64,
    cancel: CancelFlag,
    on_progress: impl Fn(u64, u64) + Send + 'static,
) -> Result<(), DownloadError> {
    let expected = expected_hex.to_ascii_lowercase();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path)?;
        let mut reader = BufReader::with_capacity(64 * 1024, file);
        let mut hasher = Sha256::new();
        let mut hashed: u64 = 0;
        let mut buf = vec![0u8; 64 * 1024];
        let mut last_emit = Instant::now();
        let mut bytes_since_emit: u64 = 0;

        loop {
            if cancel.is_cancelled() {
                return Err(DownloadError::Cancelled);
            }
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            hashed += n as u64;
            bytes_since_emit += n as u64;

            if bytes_since_emit >= PROGRESS_BYTES || last_emit.elapsed() >= PROGRESS_INTERVAL {
                on_progress(hashed, total);
                last_emit = Instant::now();
                bytes_since_emit = 0;
            }
        }

        on_progress(hashed, total);

        let got: String = hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if got.eq_ignore_ascii_case(&expected) {
            Ok(())
        } else {
            Err(DownloadError::ChecksumMismatch { expected, got })
        }
    })
    .await
    .map_err(|_| DownloadError::Io(std::io::Error::other("sha256 task panicked")))?
}

/// Download `entry.url` into `models_dir`, resuming if a `.part` file already
/// exists. After the stream completes the `.part` is SHA256-verified; on
/// mismatch it is deleted and `ChecksumMismatch` is returned.
///
/// `on_progress(downloaded, total)` fires during the download phase;
/// `on_verify_progress(hashed, total)` fires during the verification phase.
/// Both fire at most every `PROGRESS_INTERVAL` or `PROGRESS_BYTES`, whichever
/// comes first.
pub async fn download_model<F, G>(
    client: &reqwest::Client,
    entry: &ModelEntry,
    models_dir: &Path,
    cancel: CancelFlag,
    on_progress: F,
    on_verify_progress: G,
) -> Result<DownloadOutcome, DownloadError>
where
    F: Fn(u64, u64) + Send,
    G: Fn(u64, u64) + Send + 'static,
{
    fs::create_dir_all(models_dir).await?;
    let part_path = models_dir.join(format!("{}.part", entry.filename));
    let final_path = models_dir.join(&entry.filename);

    // Already complete from a previous run.
    if fs::try_exists(&final_path).await? {
        let bytes = fs::metadata(&final_path).await?.len();
        return Ok(DownloadOutcome {
            final_path,
            bytes_written: bytes,
        });
    }

    let mut existing: u64 = match fs::metadata(&part_path).await {
        Ok(m) => m.len(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
        Err(e) => return Err(e.into()),
    };

    // `.part` larger than the catalog size => corruption or stale schema.
    // Restart from zero — same effect as a fresh download.
    if existing > entry.size_bytes {
        tracing::warn!(
            existing,
            expected = entry.size_bytes,
            "stale .part larger than catalog size; restarting"
        );
        fs::remove_file(&part_path).await?;
        existing = 0;
    }

    // Optimistic: `.part` already matches the expected size. Verify before
    // promoting — a truncated-then-padded .part would pass the size check but
    // fail the hash.
    if existing == entry.size_bytes {
        on_progress(existing, entry.size_bytes);
        if let Err(e) = verify_sha256(
            part_path.clone(),
            &entry.sha256,
            entry.size_bytes,
            cancel.clone(),
            on_verify_progress,
        )
        .await
        {
            if !matches!(e, DownloadError::Cancelled) {
                fs::remove_file(&part_path).await.ok();
            }
            return Err(e);
        }
        fs::rename(&part_path, &final_path).await?;
        return Ok(DownloadOutcome {
            final_path,
            bytes_written: existing,
        });
    }

    let mut req = client.get(&entry.url);
    if existing > 0 {
        req = req.header(reqwest::header::RANGE, format!("bytes={}-", existing));
    }
    let resp = req.send().await?;
    let status = resp.status();

    // Server didn't honor our range — force restart from zero. Caller's `.part`
    // gets truncated; next chunk write begins from offset 0.
    if existing > 0 && status == reqwest::StatusCode::OK {
        tracing::warn!("server ignored Range header (status 200); restarting from zero");
        existing = 0;
        // Truncate by reopening with `.truncate(true)` below.
    } else if !status.is_success() {
        return Err(DownloadError::BadStatus(status.as_u16()));
    }

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(existing == 0)
        .append(existing > 0)
        .open(&part_path)
        .await?;

    let mut downloaded = existing;
    let total = entry.size_bytes;
    on_progress(downloaded, total);

    let mut last_emit = Instant::now();
    let mut bytes_since_emit: u64 = 0;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        if cancel.is_cancelled() {
            file.flush().await.ok();
            return Err(DownloadError::Cancelled);
        }
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        bytes_since_emit = bytes_since_emit.saturating_add(chunk.len() as u64);

        if bytes_since_emit >= PROGRESS_BYTES || last_emit.elapsed() >= PROGRESS_INTERVAL {
            on_progress(downloaded, total);
            last_emit = Instant::now();
            bytes_since_emit = 0;
        }
    }

    file.flush().await?;
    file.sync_all().await?;
    drop(file);

    if downloaded != total {
        return Err(DownloadError::SizeMismatch {
            expected: total,
            got: downloaded,
        });
    }

    on_progress(downloaded, total);

    if let Err(e) = verify_sha256(
        part_path.clone(),
        &entry.sha256,
        total,
        cancel,
        on_verify_progress,
    )
    .await
    {
        if !matches!(e, DownloadError::Cancelled) {
            fs::remove_file(&part_path).await.ok();
        }
        return Err(e);
    }

    fs::rename(&part_path, &final_path).await?;

    Ok(DownloadOutcome {
        final_path,
        bytes_written: downloaded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize};
    use std::sync::Mutex as StdMutex;

    use crate::models::catalog::ChatTemplate;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn cancel_flag_starts_unset_and_latches() {
        let f = CancelFlag::new();
        assert!(!f.is_cancelled());
        let clone = f.clone();
        clone.cancel();
        assert!(f.is_cancelled());
    }

    fn sha256_hex(data: &[u8]) -> String {
        let mut h = Sha256::new();
        h.update(data);
        h.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    fn make_entry(port: u16, name: &str, size: u64, sha256: &str) -> ModelEntry {
        ModelEntry {
            id: "test".into(),
            name: "Test".into(),
            publisher: "T".into(),
            description: "t".into(),
            license: "MIT".into(),
            tags: vec![],
            url: format!("http://127.0.0.1:{port}/{name}"),
            filename: name.into(),
            size_bytes: size,
            sha256: sha256.into(),
            params_b: 1.0,
            quantization: "Q4".into(),
            context_length: 2048,
            chat_template: ChatTemplate::ChatML,
            min_ram_mb: 1,
            recommended_ram_mb: 1,
            min_vram_mb: None,
        }
    }

    fn make_body(size: usize) -> Vec<u8> {
        (0..size).map(|i| (i % 251) as u8).collect()
    }

    fn http_client() -> reqwest::Client {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .build()
            .unwrap()
    }

    /// Hand-rolled HTTP/1.1 server controlled per-test. Keeping it in-module
    /// avoids dragging axum/hyper into dev-deps for a few hundred lines of
    /// test logic.
    #[derive(Clone, Default)]
    struct ServerOpts {
        ignore_range: Arc<AtomicBool>,
        truncate_after: Arc<AtomicU64>,
        delay_per_chunk_ms: Arc<AtomicU64>,
        last_range: Arc<tokio::sync::Mutex<String>>,
        request_count: Arc<AtomicUsize>,
    }

    async fn spawn_server(body: Vec<u8>, opts: ServerOpts) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let body = body.clone();
                let opts = opts.clone();
                tokio::spawn(async move {
                    opts.request_count.fetch_add(1, Ordering::SeqCst);
                    let mut buf = [0u8; 4096];
                    let n = match sock.read(&mut buf).await {
                        Ok(n) if n > 0 => n,
                        _ => return,
                    };
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let range = parse_range(&req);
                    {
                        let mut last = opts.last_range.lock().await;
                        *last = range.clone().unwrap_or_default();
                    }

                    let ignore = opts.ignore_range.load(Ordering::SeqCst);
                    let (status, slice): (u16, &[u8]) = match (range.as_deref(), ignore) {
                        (Some(r), false) => match parse_range_start(r) {
                            Some(start) if (start as usize) < body.len() => {
                                (206, &body[start as usize..])
                            }
                            Some(_) => (416, b""),
                            None => (400, b""),
                        },
                        _ => (200, &body[..]),
                    };

                    let truncate_after = opts.truncate_after.load(Ordering::SeqCst);
                    let send: &[u8] =
                        if truncate_after > 0 && (truncate_after as usize) < slice.len() {
                            &slice[..truncate_after as usize]
                        } else {
                            slice
                        };

                    let header = format!(
                        "HTTP/1.1 {status} {reason}\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n",
                        status = status,
                        reason = match status {
                            200 => "OK",
                            206 => "Partial Content",
                            416 => "Range Not Satisfiable",
                            _ => "Bad Request",
                        },
                        len = if status >= 400 { 0 } else { slice.len() },
                    );
                    if sock.write_all(header.as_bytes()).await.is_err() {
                        return;
                    }

                    let delay = opts.delay_per_chunk_ms.load(Ordering::SeqCst);
                    if delay == 0 {
                        let _ = sock.write_all(send).await;
                    } else {
                        for chunk in send.chunks(4096) {
                            if sock.write_all(chunk).await.is_err() {
                                return;
                            }
                            tokio::time::sleep(Duration::from_millis(delay)).await;
                        }
                    }
                    let _ = sock.shutdown().await;
                });
            }
        });
        port
    }

    fn parse_range(req: &str) -> Option<String> {
        for line in req.lines() {
            if let Some(rest) = line
                .strip_prefix("Range:")
                .or_else(|| line.strip_prefix("range:"))
            {
                return Some(rest.trim().to_owned());
            }
        }
        None
    }

    fn parse_range_start(value: &str) -> Option<u64> {
        let v = value.strip_prefix("bytes=")?.trim();
        let (start, _end) = v.split_once('-')?;
        start.trim().parse().ok()
    }

    #[tokio::test]
    async fn fresh_download_writes_full_file_and_emits_progress() {
        let body = make_body(64 * 1024);
        let opts = ServerOpts::default();
        opts.delay_per_chunk_ms.store(1, Ordering::SeqCst);
        let port = spawn_server(body.clone(), opts.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        let entry = make_entry(port, "model.gguf", body.len() as u64, &sha256_hex(&body));

        let progress = Arc::new(StdMutex::new(Vec::<(u64, u64)>::new()));
        let p2 = progress.clone();
        let on_progress = move |d, t| {
            p2.lock().unwrap().push((d, t));
        };

        let outcome = download_model(
            &http_client(),
            &entry,
            dir.path(),
            CancelFlag::new(),
            on_progress,
            |_, _| {},
        )
        .await
        .unwrap();

        assert_eq!(outcome.bytes_written, body.len() as u64);
        let final_bytes = std::fs::read(&outcome.final_path).unwrap();
        assert_eq!(final_bytes, body);
        assert!(!dir.path().join("model.gguf.part").exists());

        let snapshot = progress.lock().unwrap().clone();
        assert!(!snapshot.is_empty());
        assert_eq!(
            snapshot.last().unwrap(),
            &(body.len() as u64, body.len() as u64)
        );
        assert!(snapshot.iter().all(|(d, t)| *d <= *t));
        assert!(snapshot.windows(2).all(|w| w[0].0 <= w[1].0));
        assert_eq!(opts.last_range.lock().await.clone(), "");
    }

    #[tokio::test]
    async fn resume_uses_range_header_and_appends_remaining_bytes() {
        let body = make_body(32 * 1024);
        let opts = ServerOpts::default();
        let port = spawn_server(body.clone(), opts.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        let entry = make_entry(port, "model.gguf", body.len() as u64, &sha256_hex(&body));

        let half = body.len() / 2;
        std::fs::write(dir.path().join("model.gguf.part"), &body[..half]).unwrap();

        let outcome = download_model(
            &http_client(),
            &entry,
            dir.path(),
            CancelFlag::new(),
            |_, _| {},
            |_, _| {},
        )
        .await
        .unwrap();

        assert_eq!(
            opts.last_range.lock().await.clone(),
            format!("bytes={half}-")
        );
        assert_eq!(outcome.bytes_written, body.len() as u64);
        assert_eq!(std::fs::read(&outcome.final_path).unwrap(), body);
    }

    #[tokio::test]
    async fn part_already_complete_short_circuits_to_rename() {
        let body = make_body(8 * 1024);
        let opts = ServerOpts::default();
        let port = spawn_server(body.clone(), opts.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        let entry = make_entry(port, "model.gguf", body.len() as u64, &sha256_hex(&body));

        let part_path = dir.path().join("model.gguf.part");
        std::fs::write(&part_path, &body).unwrap();

        let outcome = download_model(
            &http_client(),
            &entry,
            dir.path(),
            CancelFlag::new(),
            |_, _| {},
            |_, _| {},
        )
        .await
        .unwrap();

        assert_eq!(outcome.bytes_written, body.len() as u64);
        assert!(outcome.final_path.exists());
        assert!(!part_path.exists());
        assert_eq!(opts.request_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn final_file_present_short_circuits_with_zero_traffic() {
        let body = make_body(8 * 1024);
        let opts = ServerOpts::default();
        let port = spawn_server(body.clone(), opts.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        // sha256 irrelevant: returns early when final file exists, no verify
        let entry = make_entry(port, "model.gguf", body.len() as u64, &"a".repeat(64));

        std::fs::write(dir.path().join("model.gguf"), &body).unwrap();

        let outcome = download_model(
            &http_client(),
            &entry,
            dir.path(),
            CancelFlag::new(),
            |_, _| {},
            |_, _| {},
        )
        .await
        .unwrap();

        assert_eq!(outcome.bytes_written, body.len() as u64);
        assert_eq!(opts.request_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn part_larger_than_expected_restarts_from_zero() {
        let body = make_body(8 * 1024);
        let opts = ServerOpts::default();
        let port = spawn_server(body.clone(), opts.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        let entry = make_entry(port, "model.gguf", body.len() as u64, &sha256_hex(&body));

        std::fs::write(
            dir.path().join("model.gguf.part"),
            vec![0xFFu8; body.len() * 2],
        )
        .unwrap();

        let outcome = download_model(
            &http_client(),
            &entry,
            dir.path(),
            CancelFlag::new(),
            |_, _| {},
            |_, _| {},
        )
        .await
        .unwrap();

        assert_eq!(opts.last_range.lock().await.clone(), "");
        assert_eq!(std::fs::read(&outcome.final_path).unwrap(), body);
    }

    #[tokio::test]
    async fn server_ignores_range_and_returns_200_restarts_from_zero() {
        let body = make_body(8 * 1024);
        let opts = ServerOpts::default();
        opts.ignore_range.store(true, Ordering::SeqCst);
        let port = spawn_server(body.clone(), opts.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        let entry = make_entry(port, "model.gguf", body.len() as u64, &sha256_hex(&body));

        let half = body.len() / 2;
        std::fs::write(dir.path().join("model.gguf.part"), &body[..half]).unwrap();

        let outcome = download_model(
            &http_client(),
            &entry,
            dir.path(),
            CancelFlag::new(),
            |_, _| {},
            |_, _| {},
        )
        .await
        .unwrap();

        assert_eq!(std::fs::read(&outcome.final_path).unwrap(), body);
        assert_eq!(outcome.bytes_written, body.len() as u64);
    }

    #[tokio::test]
    async fn cancellation_mid_download_returns_cancelled_and_keeps_part() {
        let body = make_body(256 * 1024);
        let opts = ServerOpts::default();
        opts.delay_per_chunk_ms.store(20, Ordering::SeqCst);
        let port = spawn_server(body.clone(), opts.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        // sha256 irrelevant: download cancelled before verification
        let entry = make_entry(port, "model.gguf", body.len() as u64, &"a".repeat(64));

        let cancel = CancelFlag::new();
        let cancel_h = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(40)).await;
            cancel_h.cancel();
        });

        let err = download_model(
            &http_client(),
            &entry,
            dir.path(),
            cancel,
            |_, _| {},
            |_, _| {},
        )
        .await
        .unwrap_err();

        assert!(matches!(err, DownloadError::Cancelled), "got {err:?}");
        let part_path = dir.path().join("model.gguf.part");
        assert!(part_path.exists());
        let part_size = std::fs::metadata(&part_path).unwrap().len();
        assert!(part_size > 0 && part_size < body.len() as u64);
        assert!(!dir.path().join("model.gguf").exists());
    }

    #[tokio::test]
    async fn truncated_response_keeps_part_for_resume() {
        let body = make_body(8 * 1024);
        let opts = ServerOpts::default();
        opts.truncate_after.store(2 * 1024, Ordering::SeqCst);
        let port = spawn_server(body.clone(), opts.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        // sha256 irrelevant: download fails before verification
        let entry = make_entry(port, "model.gguf", body.len() as u64, &"a".repeat(64));

        let err = download_model(
            &http_client(),
            &entry,
            dir.path(),
            CancelFlag::new(),
            |_, _| {},
            |_, _| {},
        )
        .await
        .unwrap_err();

        assert!(
            matches!(
                err,
                DownloadError::SizeMismatch { .. } | DownloadError::Http(_)
            ),
            "got {err:?}"
        );
        assert!(dir.path().join("model.gguf.part").exists());
        assert!(!dir.path().join("model.gguf").exists());
    }

    #[tokio::test]
    async fn drop_resume_cycle_eventually_completes_full_file() {
        let body = make_body(16 * 1024);
        let opts = ServerOpts::default();
        opts.truncate_after.store(6 * 1024, Ordering::SeqCst);
        opts.delay_per_chunk_ms.store(1, Ordering::SeqCst);
        let port = spawn_server(body.clone(), opts.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        let entry = make_entry(port, "model.gguf", body.len() as u64, &sha256_hex(&body));

        let _ = download_model(
            &http_client(),
            &entry,
            dir.path(),
            CancelFlag::new(),
            |_, _| {},
            |_, _| {},
        )
        .await;
        let part_path = dir.path().join("model.gguf.part");
        assert!(part_path.exists());
        let after_first = std::fs::metadata(&part_path).unwrap().len();
        assert!(after_first > 0 && after_first < body.len() as u64);

        opts.truncate_after.store(0, Ordering::SeqCst);

        let outcome = download_model(
            &http_client(),
            &entry,
            dir.path(),
            CancelFlag::new(),
            |_, _| {},
            |_, _| {},
        )
        .await
        .expect("second attempt completes");

        assert_eq!(std::fs::read(&outcome.final_path).unwrap(), body);
        assert_eq!(
            opts.last_range.lock().await.clone(),
            format!("bytes={after_first}-")
        );
    }

    #[tokio::test]
    async fn unreachable_server_surfaces_http_error() {
        let dir = tempfile::tempdir().unwrap();
        let entry = make_entry(1, "missing.gguf", 1024, &"a".repeat(64));
        let err = download_model(
            &http_client(),
            &entry,
            dir.path(),
            CancelFlag::new(),
            |_, _| {},
            |_, _| {},
        )
        .await
        .unwrap_err();
        assert!(matches!(err, DownloadError::Http(_)), "got {err:?}");
    }

    // --- Fase 4.4: SHA256 integrity tests ---

    #[tokio::test]
    async fn checksum_mismatch_deletes_part_and_returns_error() {
        let body = make_body(16 * 1024);
        let opts = ServerOpts::default();
        let port = spawn_server(body.clone(), opts.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        // Wrong hash — 64 hex 'a's will never match real content
        let entry = make_entry(port, "model.gguf", body.len() as u64, &"a".repeat(64));

        let err = download_model(
            &http_client(),
            &entry,
            dir.path(),
            CancelFlag::new(),
            |_, _| {},
            |_, _| {},
        )
        .await
        .unwrap_err();

        assert!(
            matches!(err, DownloadError::ChecksumMismatch { .. }),
            "expected ChecksumMismatch, got {err:?}"
        );
        // .part deleted on mismatch; final file never created
        assert!(!dir.path().join("model.gguf.part").exists());
        assert!(!dir.path().join("model.gguf").exists());
    }

    #[tokio::test]
    async fn checksum_mismatch_reports_expected_and_actual_hashes() {
        let body = make_body(8 * 1024);
        let opts = ServerOpts::default();
        let port = spawn_server(body.clone(), opts.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        let expected_in_catalog = "b".repeat(64);
        let entry = make_entry(port, "model.gguf", body.len() as u64, &expected_in_catalog);

        let err = download_model(
            &http_client(),
            &entry,
            dir.path(),
            CancelFlag::new(),
            |_, _| {},
            |_, _| {},
        )
        .await
        .unwrap_err();

        if let DownloadError::ChecksumMismatch { expected, got } = err {
            assert_eq!(expected, expected_in_catalog);
            assert_eq!(got, sha256_hex(&body));
        } else {
            panic!("expected ChecksumMismatch, got {err:?}");
        }
    }

    #[tokio::test]
    async fn checksum_uppercase_catalog_matches_lowercase_digest() {
        let body = make_body(8 * 1024);
        let opts = ServerOpts::default();
        let port = spawn_server(body.clone(), opts.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        // Catalog stores uppercase (as enforced by catalog validation)
        let uppercase_hash = sha256_hex(&body).to_ascii_uppercase();
        let entry = make_entry(port, "model.gguf", body.len() as u64, &uppercase_hash);

        download_model(
            &http_client(),
            &entry,
            dir.path(),
            CancelFlag::new(),
            |_, _| {},
            |_, _| {},
        )
        .await
        .expect("uppercase hash should match lowercase digest");

        assert!(dir.path().join("model.gguf").exists());
    }

    #[tokio::test]
    async fn verify_progress_callback_fires_monotonically() {
        let body = make_body(64 * 1024);
        let opts = ServerOpts::default();
        let port = spawn_server(body.clone(), opts.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        let entry = make_entry(port, "model.gguf", body.len() as u64, &sha256_hex(&body));

        let verify_calls = Arc::new(StdMutex::new(Vec::<(u64, u64)>::new()));
        let vc2 = verify_calls.clone();

        download_model(
            &http_client(),
            &entry,
            dir.path(),
            CancelFlag::new(),
            |_, _| {},
            move |hashed, total| {
                vc2.lock().unwrap().push((hashed, total));
            },
        )
        .await
        .unwrap();

        let calls = verify_calls.lock().unwrap().clone();
        assert!(
            !calls.is_empty(),
            "verify progress should fire at least once"
        );
        let total = body.len() as u64;
        // Last call must report full file hashed
        assert_eq!(calls.last().unwrap(), &(total, total));
        // Monotonically increasing hashed bytes
        assert!(calls.windows(2).all(|w| w[0].0 <= w[1].0));
        // total_bytes consistent
        assert!(calls.iter().all(|(_, t)| *t == total));
    }

    #[tokio::test]
    async fn optimistic_complete_part_with_wrong_hash_deletes_part() {
        let body = make_body(8 * 1024);
        let opts = ServerOpts::default();
        let port = spawn_server(body.clone(), opts.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        // Wrong hash so optimistic branch fails verification
        let entry = make_entry(port, "model.gguf", body.len() as u64, &"c".repeat(64));

        let part_path = dir.path().join("model.gguf.part");
        std::fs::write(&part_path, &body).unwrap();

        let err = download_model(
            &http_client(),
            &entry,
            dir.path(),
            CancelFlag::new(),
            |_, _| {},
            |_, _| {},
        )
        .await
        .unwrap_err();

        assert!(
            matches!(err, DownloadError::ChecksumMismatch { .. }),
            "got {err:?}"
        );
        // .part deleted even in optimistic path
        assert!(!part_path.exists());
        assert!(!dir.path().join("model.gguf").exists());
        // No HTTP traffic — optimistic branch short-circuits
        assert_eq!(opts.request_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn cancel_during_verify_keeps_part_for_resume() {
        // Use a large enough body that hashing takes non-zero time. The cancel
        // flag is set synchronously before calling download_model, simulating a
        // cancel that arrives just as verification starts.
        let body = make_body(8 * 1024 * 1024); // 8 MiB
        let opts = ServerOpts::default();
        let port = spawn_server(body.clone(), opts.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        let entry = make_entry(port, "model.gguf", body.len() as u64, &sha256_hex(&body));

        // Write full .part so we exercise the optimistic branch (no HTTP).
        let part_path = dir.path().join("model.gguf.part");
        std::fs::write(&part_path, &body).unwrap();

        let cancel = CancelFlag::new();
        cancel.cancel(); // cancelled before download_model even starts

        let err = download_model(
            &http_client(),
            &entry,
            dir.path(),
            cancel,
            |_, _| {},
            |_, _| {},
        )
        .await
        .unwrap_err();

        assert!(matches!(err, DownloadError::Cancelled), "got {err:?}");
        // .part retained on cancel so the user can resume
        assert!(part_path.exists());
        assert!(!dir.path().join("model.gguf").exists());
    }
}
