//! Resolving a user-pasted model link into downloadable GGUF files.
//!
//! The user types (or pastes) one of:
//!
//! * a Hugging Face repo — `https://huggingface.co/owner/repo`, `hf.co/…`,
//!   or the bare `owner/repo` — which usually holds several quantizations;
//! * a Hugging Face file page or download link —
//!   `…/owner/repo/blob/main/x.gguf` or `…/resolve/main/x.gguf`;
//! * any other `https://` URL that ends in `.gguf`.
//!
//! Repos are expanded through the public tree API
//! (`GET /api/models/{repo}/tree/{revision}?recursive=true`), which lists
//! every file with its size — enough for the UI to show the choices and for
//! the download to report progress. Nothing here needs authentication.

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

pub const HF_HOST: &str = "https://huggingface.co";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceRef {
    /// `owner/repo` plus the git revision to read (`main` unless the link
    /// pinned another).
    Repo { repo: String, revision: String },
    /// One file inside a repo, by its path within the repo.
    File {
        repo: String,
        revision: String,
        path: String,
    },
    /// A direct link outside Hugging Face.
    Direct { url: String },
}

/// A GGUF that can be downloaded, as presented to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct RemoteGguf {
    /// File name the download will be saved under.
    pub filename: String,
    /// Direct download URL.
    pub url: String,
    /// `None` when the server didn't say (direct links without
    /// `Content-Length`).
    pub size_bytes: Option<u64>,
    /// Shard of a multi-file GGUF (`-00001-of-00003.gguf`). Listed so the
    /// user understands why it's greyed out; not downloadable.
    pub split: bool,
    /// Fit against the detected hardware, estimated from the file size.
    /// Filled in by the command layer; `None` when the size is unknown.
    pub tier: Option<crate::models::compatibility::CompatibilityTier>,
}

/// What a pasted link resolved to.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct ResolvedSource {
    /// `owner/repo` for Hugging Face sources; `None` for direct links.
    pub repo: Option<String>,
    /// The normalised link the row will remember as its `source_url`.
    pub source_url: String,
    pub files: Vec<RemoteGguf>,
}

#[derive(Debug, Error)]
pub enum HfError {
    #[error("that doesn't look like a Hugging Face repo or a .gguf link")]
    Unrecognised,
    #[error("Hugging Face request failed: {0}")]
    Http(String),
    #[error("Hugging Face returned status {0}")]
    BadStatus(u16),
    #[error("could not read the Hugging Face file list: {0}")]
    Parse(String),
    #[error("this repository has no GGUF files")]
    NoGguf,
}

impl From<reqwest::Error> for HfError {
    fn from(e: reqwest::Error) -> Self {
        HfError::Http(e.to_string())
    }
}

/// Classify a user-typed link. Pure; no network.
pub fn parse_source(input: &str) -> Result<SourceRef, HfError> {
    let trimmed = input.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(HfError::Unrecognised);
    }

    // Strip scheme + HF host, leaving the path inside the site.
    let hf_path = [
        "https://huggingface.co/",
        "http://huggingface.co/",
        "https://hf.co/",
        "http://hf.co/",
    ]
    .iter()
    .find_map(|prefix| trimmed.strip_prefix(prefix))
    .or_else(|| {
        // Bare `owner/repo` (no scheme, no dots in the first segment).
        let looks_bare = !trimmed.contains("://")
            && trimmed.matches('/').count() == 1
            && !trimmed.split('/').next().unwrap_or("").contains('.');
        looks_bare.then_some(trimmed)
    });

    if let Some(path) = hf_path {
        let path = path.split(['?', '#']).next().unwrap_or("");
        let mut parts = path.split('/').filter(|p| !p.is_empty());
        let owner = parts.next().ok_or(HfError::Unrecognised)?;
        let name = parts.next().ok_or(HfError::Unrecognised)?;
        let repo = format!("{owner}/{name}");
        if !valid_repo_segment(owner) || !valid_repo_segment(name) {
            return Err(HfError::Unrecognised);
        }

        let rest: Vec<&str> = parts.collect();
        return Ok(match rest.as_slice() {
            [] => SourceRef::Repo {
                repo,
                revision: "main".into(),
            },
            ["tree", revision, ..] => SourceRef::Repo {
                repo,
                revision: (*revision).to_owned(),
            },
            [kind, revision, file @ ..]
                if (*kind == "blob" || *kind == "resolve") && !file.is_empty() =>
            {
                let path = file.join("/");
                if !is_gguf(&path) {
                    return Err(HfError::Unrecognised);
                }
                SourceRef::File {
                    repo,
                    revision: (*revision).to_owned(),
                    path,
                }
            }
            _ => return Err(HfError::Unrecognised),
        });
    }

    if trimmed.starts_with("https://") {
        let path = trimmed.split(['?', '#']).next().unwrap_or("");
        if is_gguf(path) {
            return Ok(SourceRef::Direct {
                url: trimmed.to_owned(),
            });
        }
    }

    Err(HfError::Unrecognised)
}

fn valid_repo_segment(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn is_gguf(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".gguf")
}

/// `x-00001-of-00003.gguf` and friends.
pub fn is_split_shard(filename: &str) -> bool {
    let stem = filename.strip_suffix(".gguf").unwrap_or(filename);
    let Some((head, count)) = stem.rsplit_once("-of-") else {
        return false;
    };
    let Some((_, no)) = head.rsplit_once('-') else {
        return false;
    };
    let five_digits = |s: &str| s.len() == 5 && s.bytes().all(|b| b.is_ascii_digit());
    five_digits(no) && five_digits(count)
}

pub fn resolve_url(repo: &str, revision: &str, path: &str) -> String {
    format!("{HF_HOST}/{repo}/resolve/{revision}/{path}")
}

fn filename_of(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_owned()
}

#[derive(Deserialize)]
struct TreeEntry {
    #[serde(rename = "type")]
    kind: String,
    path: String,
    size: Option<u64>,
}

/// Expand `source` into the list of downloadable files.
pub async fn resolve(
    client: &reqwest::Client,
    source: &SourceRef,
) -> Result<ResolvedSource, HfError> {
    match source {
        SourceRef::Repo { repo, revision } => {
            let entries: Vec<TreeEntry> = client
                .get(format!(
                    "{HF_HOST}/api/models/{repo}/tree/{revision}?recursive=true"
                ))
                .send()
                .await?
                .error_for_status()
                .map_err(|e| match e.status() {
                    Some(s) => HfError::BadStatus(s.as_u16()),
                    None => HfError::Http(e.to_string()),
                })?
                .json()
                .await
                .map_err(|e| HfError::Parse(e.to_string()))?;

            let mut files: Vec<RemoteGguf> = entries
                .into_iter()
                .filter(|e| e.kind == "file" && is_gguf(&e.path))
                .map(|e| {
                    let filename = filename_of(&e.path);
                    RemoteGguf {
                        split: is_split_shard(&filename),
                        url: resolve_url(repo, revision, &e.path),
                        size_bytes: e.size,
                        filename,
                        tier: None,
                    }
                })
                .collect();
            if files.is_empty() {
                return Err(HfError::NoGguf);
            }
            // Smallest first: the casual user's machine is more likely to fit
            // the lighter quantizations, and they read top-down.
            files.sort_by_key(|f| (f.split, f.size_bytes.unwrap_or(u64::MAX)));

            Ok(ResolvedSource {
                repo: Some(repo.clone()),
                source_url: format!("{HF_HOST}/{repo}"),
                files,
            })
        }
        SourceRef::File {
            repo,
            revision,
            path,
        } => {
            let url = resolve_url(repo, revision, path);
            let filename = filename_of(path);
            let size_bytes = probe_size(client, &url).await;
            Ok(ResolvedSource {
                repo: Some(repo.clone()),
                source_url: format!("{HF_HOST}/{repo}/blob/{revision}/{path}"),
                files: vec![RemoteGguf {
                    split: is_split_shard(&filename),
                    filename,
                    url,
                    size_bytes,
                    tier: None,
                }],
            })
        }
        SourceRef::Direct { url } => {
            let filename = filename_of(url.split(['?', '#']).next().unwrap_or(url));
            let size_bytes = probe_size(client, url).await;
            Ok(ResolvedSource {
                repo: None,
                source_url: url.clone(),
                files: vec![RemoteGguf {
                    split: is_split_shard(&filename),
                    filename,
                    url: url.clone(),
                    size_bytes,
                    tier: None,
                }],
            })
        }
    }
}

/// Size announced by a `HEAD` on the download URL. Best-effort: `None` when
/// the server doesn't answer HEAD or omits `Content-Length` — the download
/// itself will discover the size from the GET.
async fn probe_size(client: &reqwest::Client, url: &str) -> Option<u64> {
    let resp = client.head(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    // HF answers HEAD on `/resolve/` with a redirect to the CDN that reqwest
    // follows; the LFS size also travels in `x-linked-size` when
    // `Content-Length` describes the redirect body instead.
    resp.headers()
        .get("x-linked-size")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .or_else(|| resp.content_length())
        .filter(|n| *n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_repo_urls() {
        for input in [
            "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF",
            "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/",
            "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/tree/main",
            "https://hf.co/bartowski/Llama-3.2-3B-Instruct-GGUF?not-for-all-audiences=true",
            "bartowski/Llama-3.2-3B-Instruct-GGUF",
        ] {
            assert_eq!(
                parse_source(input).unwrap(),
                SourceRef::Repo {
                    repo: "bartowski/Llama-3.2-3B-Instruct-GGUF".into(),
                    revision: "main".into(),
                },
                "{input}"
            );
        }
    }

    #[test]
    fn parses_file_urls_with_nested_paths_and_revisions() {
        let blob = parse_source(
            "https://huggingface.co/unsloth/Qwen3-8B-GGUF/blob/main/Qwen3-8B-Q4_K_M.gguf",
        )
        .unwrap();
        assert_eq!(
            blob,
            SourceRef::File {
                repo: "unsloth/Qwen3-8B-GGUF".into(),
                revision: "main".into(),
                path: "Qwen3-8B-Q4_K_M.gguf".into(),
            }
        );
        let resolve =
            parse_source("https://huggingface.co/o/r/resolve/v2/sub/dir/m.GGUF?download=true")
                .unwrap();
        assert_eq!(
            resolve,
            SourceRef::File {
                repo: "o/r".into(),
                revision: "v2".into(),
                path: "sub/dir/m.GGUF".into(),
            }
        );
    }

    #[test]
    fn parses_direct_gguf_links() {
        assert_eq!(
            parse_source("https://example.com/models/x.gguf?token=1").unwrap(),
            SourceRef::Direct {
                url: "https://example.com/models/x.gguf?token=1".into()
            }
        );
    }

    #[test]
    fn rejects_everything_else() {
        for input in [
            "",
            "hello",
            "https://example.com/readme.md",
            "http://example.com/x.gguf", // plain http outside HF
            "https://huggingface.co/",
            "https://huggingface.co/only-owner",
            "https://huggingface.co/o/r/blob/main/not-a-gguf.bin",
            "https://huggingface.co/o/r/discussions",
            "owner/repo/extra",
        ] {
            assert!(
                matches!(parse_source(input), Err(HfError::Unrecognised)),
                "{input}"
            );
        }
    }

    #[test]
    fn detects_split_shards() {
        assert!(is_split_shard("big-00001-of-00003.gguf"));
        assert!(is_split_shard("Big-Model-Q8_0-00002-of-00002.gguf"));
        assert!(!is_split_shard("model-Q4_K_M.gguf"));
        assert!(!is_split_shard("model-of-the-year.gguf"));
    }

    #[test]
    fn builds_resolve_urls() {
        assert_eq!(
            resolve_url("o/r", "main", "sub/m.gguf"),
            "https://huggingface.co/o/r/resolve/main/sub/m.gguf"
        );
    }
}
