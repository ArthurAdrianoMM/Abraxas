//! Remote model catalog (Fase 4.1).
//!
//! Fetches the published JSON catalog from GitHub Pages, validates it against
//! the v1 schema, caches the validated payload to disk, and falls back to the
//! cache when the network is unreachable.
//!
//! Network-first by design: every call attempts a fresh fetch so newly
//! published models reach the user without an explicit refresh action. The
//! disk cache (`catalog_cache.json` in `app_data_dir`) only kicks in when the
//! HTTP request fails, keeping the app usable offline.
//!
//! Schema fields are intentionally richer than what 4.1 itself consumes — the
//! catalog is a contract with the published JSON file, and adding fields
//! later means breaking either the parser or every deployed catalog version.
//! See `plan-the-fase-4-1-warm-lampson.md` for the per-field rationale.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub const CATALOG_URL: &str = "https://arthuradrianomm.github.io/Abraxas/catalog.json";
pub const CATALOG_CACHE_FILENAME: &str = "catalog_cache.json";
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(15);
pub const SUPPORTED_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Catalog {
    pub schema_version: u8,
    pub generated_at: String,
    pub models: Vec<ModelEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ModelEntry {
    pub id: String,
    pub name: String,
    pub publisher: String,
    pub description: String,
    pub license: String,
    pub tags: Vec<String>,
    pub url: String,
    pub filename: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub params_b: f32,
    pub quantization: String,
    pub context_length: u32,
    pub chat_template: ChatTemplate,
    pub min_ram_mb: u64,
    pub recommended_ram_mb: u64,
    pub min_vram_mb: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
pub enum ChatTemplate {
    Llama3,
    ChatML,
    Mistral,
    Gemma,
    Gemma4,
    Qwen,
    Qwen3,
    Phi3,
    DeepSeek,
    Llama2,
    CommandR,
    GLM4,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CatalogSource {
    Network,
    Cache,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct CatalogResponse {
    pub catalog: Catalog,
    pub source: CatalogSource,
    pub fetched_at: String,
}

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("catalog HTTP request failed: {0}")]
    Http(String),
    #[error("catalog JSON parse failed: {0}")]
    Parse(String),
    #[error("unsupported catalog schema_version {found}, expected {expected}")]
    UnsupportedSchema { found: u8, expected: u8 },
    #[error("catalog validation failed: {0}")]
    Validation(String),
    #[error("catalog cache I/O: {0}")]
    CacheIo(#[from] std::io::Error),
    #[error("network failed and no cached catalog available: {0}")]
    NoFallback(String),
}

impl From<reqwest::Error> for CatalogError {
    fn from(e: reqwest::Error) -> Self {
        CatalogError::Http(e.to_string())
    }
}

impl From<serde_json::Error> for CatalogError {
    fn from(e: serde_json::Error) -> Self {
        CatalogError::Parse(e.to_string())
    }
}

pub async fn fetch_remote(client: &reqwest::Client, url: &str) -> Result<Catalog, CatalogError> {
    let body = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let catalog: Catalog = serde_json::from_str(&body)?;
    validate(&catalog)?;
    Ok(catalog)
}

pub fn validate(c: &Catalog) -> Result<(), CatalogError> {
    if c.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(CatalogError::UnsupportedSchema {
            found: c.schema_version,
            expected: SUPPORTED_SCHEMA_VERSION,
        });
    }

    let mut seen_ids = std::collections::HashSet::with_capacity(c.models.len());
    for (idx, m) in c.models.iter().enumerate() {
        let where_ = format!("models[{idx}] (id={:?})", m.id);

        if m.id.is_empty() {
            return Err(CatalogError::Validation(format!("{where_}: id is empty")));
        }
        if m.id.chars().any(|ch| ch.is_whitespace()) {
            return Err(CatalogError::Validation(format!(
                "{where_}: id contains whitespace"
            )));
        }
        if !seen_ids.insert(&m.id) {
            return Err(CatalogError::Validation(format!("{where_}: duplicate id")));
        }

        if m.name.is_empty() {
            return Err(CatalogError::Validation(format!("{where_}: name is empty")));
        }
        if m.filename.is_empty() {
            return Err(CatalogError::Validation(format!(
                "{where_}: filename is empty"
            )));
        }
        if !m.url.starts_with("https://") {
            return Err(CatalogError::Validation(format!(
                "{where_}: url must start with https://"
            )));
        }
        if m.size_bytes == 0 {
            return Err(CatalogError::Validation(format!(
                "{where_}: size_bytes must be > 0"
            )));
        }
        if m.sha256.len() != 64 || !m.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(CatalogError::Validation(format!(
                "{where_}: sha256 must be 64 hex chars"
            )));
        }
        if m.min_ram_mb > m.recommended_ram_mb {
            return Err(CatalogError::Validation(format!(
                "{where_}: min_ram_mb ({}) > recommended_ram_mb ({})",
                m.min_ram_mb, m.recommended_ram_mb
            )));
        }
    }

    Ok(())
}

pub fn read_cache(path: &Path) -> Result<Option<Catalog>, CatalogError> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let catalog: Catalog = serde_json::from_str(&text)?;
            // Cache content was validated before write, but a future schema
            // bump would invalidate older caches — re-validate defensively.
            validate(&catalog)?;
            Ok(Some(catalog))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn write_cache(path: &Path, c: &Catalog) -> Result<(), CatalogError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(c)?;
    std::fs::write(path, text)?;
    Ok(())
}

pub async fn fetch_with_cache(
    client: &reqwest::Client,
    url: &str,
    cache_path: &Path,
) -> Result<CatalogResponse, CatalogError> {
    let now = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned());

    match fetch_remote(client, url).await {
        Ok(catalog) => {
            // Best-effort cache write. If it fails (e.g. read-only volume),
            // log and continue — the user still has fresh data this session.
            if let Err(e) = write_cache(cache_path, &catalog) {
                tracing::warn!(error = %e, "failed to write catalog cache");
            }
            Ok(CatalogResponse {
                catalog,
                source: CatalogSource::Network,
                fetched_at: now,
            })
        }
        Err(net_err) => {
            tracing::warn!(error = %net_err, "catalog network fetch failed; trying cache");
            match read_cache(cache_path)? {
                Some(catalog) => Ok(CatalogResponse {
                    catalog,
                    source: CatalogSource::Cache,
                    fetched_at: now,
                }),
                None => Err(CatalogError::NoFallback(net_err.to_string())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str) -> ModelEntry {
        ModelEntry {
            id: id.to_owned(),
            name: "Test Model".into(),
            publisher: "Tester".into(),
            description: "test".into(),
            license: "MIT".into(),
            tags: vec!["small".into()],
            url: "https://example.com/model.gguf".into(),
            filename: "model.gguf".into(),
            size_bytes: 1024,
            sha256: "a".repeat(64),
            params_b: 1.0,
            quantization: "Q4_K_M".into(),
            context_length: 2048,
            chat_template: ChatTemplate::ChatML,
            min_ram_mb: 2048,
            recommended_ram_mb: 4096,
            min_vram_mb: None,
        }
    }

    fn catalog(entries: Vec<ModelEntry>) -> Catalog {
        Catalog {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            generated_at: "2026-04-28T00:00:00Z".into(),
            models: entries,
        }
    }

    #[test]
    fn validate_accepts_well_formed_catalog() {
        let c = catalog(vec![entry("a"), entry("b")]);
        assert!(validate(&c).is_ok());
    }

    #[test]
    fn validate_rejects_duplicate_ids() {
        let c = catalog(vec![entry("dup"), entry("dup")]);
        let err = validate(&c).unwrap_err();
        match err {
            CatalogError::Validation(msg) => assert!(msg.contains("duplicate id"), "got: {msg}"),
            other => panic!("expected Validation, got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_bad_sha256_length() {
        let mut e = entry("a");
        e.sha256 = "abc".into();
        let err = validate(&catalog(vec![e])).unwrap_err();
        assert!(matches!(err, CatalogError::Validation(_)));
    }

    #[test]
    fn validate_rejects_non_hex_sha256() {
        let mut e = entry("a");
        e.sha256 = "z".repeat(64);
        let err = validate(&catalog(vec![e])).unwrap_err();
        assert!(matches!(err, CatalogError::Validation(_)));
    }

    #[test]
    fn validate_rejects_min_ram_above_recommended() {
        let mut e = entry("a");
        e.min_ram_mb = 8192;
        e.recommended_ram_mb = 4096;
        let err = validate(&catalog(vec![e])).unwrap_err();
        match err {
            CatalogError::Validation(msg) => assert!(msg.contains("min_ram_mb")),
            other => panic!("expected Validation, got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_unsupported_schema_version() {
        let mut c = catalog(vec![entry("a")]);
        c.schema_version = 99;
        let err = validate(&c).unwrap_err();
        assert!(matches!(
            err,
            CatalogError::UnsupportedSchema {
                found: 99,
                expected: 1
            }
        ));
    }

    #[test]
    fn validate_rejects_non_https_url() {
        let mut e = entry("a");
        e.url = "http://example.com/model.gguf".into();
        let err = validate(&catalog(vec![e])).unwrap_err();
        assert!(matches!(err, CatalogError::Validation(_)));
    }

    #[test]
    fn validate_rejects_zero_size() {
        let mut e = entry("a");
        e.size_bytes = 0;
        let err = validate(&catalog(vec![e])).unwrap_err();
        assert!(matches!(err, CatalogError::Validation(_)));
    }

    #[test]
    fn validate_rejects_id_with_whitespace() {
        let e = entry("has space");
        let err = validate(&catalog(vec![e])).unwrap_err();
        assert!(matches!(err, CatalogError::Validation(_)));
    }

    #[test]
    fn read_cache_returns_none_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.json");
        assert!(read_cache(&path).unwrap().is_none());
    }

    #[test]
    fn write_then_read_cache_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("cache.json");
        let original = catalog(vec![entry("rt-1"), entry("rt-2")]);
        write_cache(&path, &original).unwrap();
        let loaded = read_cache(&path).unwrap().expect("cache should exist");
        assert_eq!(loaded.models.len(), 2);
        assert_eq!(loaded.models[0].id, "rt-1");
        assert_eq!(loaded.schema_version, SUPPORTED_SCHEMA_VERSION);
    }

    #[test]
    fn parse_unknown_chat_template_fails_entry() {
        let json = r#"{
            "schema_version": 1,
            "generated_at": "2026-04-28T00:00:00Z",
            "models": [{
                "id": "x",
                "name": "X",
                "publisher": "p",
                "description": "d",
                "license": "MIT",
                "tags": [],
                "url": "https://example.com/x.gguf",
                "filename": "x.gguf",
                "size_bytes": 1,
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "params_b": 1.0,
                "quantization": "Q4_K_M",
                "context_length": 2048,
                "chat_template": "GarbageNotARealTemplate",
                "min_ram_mb": 1,
                "recommended_ram_mb": 1,
                "min_vram_mb": null
            }]
        }"#;
        let result: Result<Catalog, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "unknown chat_template variant should fail to parse"
        );
    }

    #[test]
    fn chat_template_deserializes_all_supported_names() {
        let names = [
            ("Llama3", ChatTemplate::Llama3),
            ("ChatML", ChatTemplate::ChatML),
            ("Mistral", ChatTemplate::Mistral),
            ("Gemma", ChatTemplate::Gemma),
            ("Gemma4", ChatTemplate::Gemma4),
            ("Qwen", ChatTemplate::Qwen),
            ("Qwen3", ChatTemplate::Qwen3),
            ("Phi3", ChatTemplate::Phi3),
            ("DeepSeek", ChatTemplate::DeepSeek),
            ("Llama2", ChatTemplate::Llama2),
            ("CommandR", ChatTemplate::CommandR),
            ("GLM4", ChatTemplate::GLM4),
        ];

        for (name, expected) in names {
            let parsed: ChatTemplate = serde_json::from_str(&format!(r#""{name}""#)).unwrap();
            assert_eq!(parsed, expected, "{name} should parse");
        }
    }

    #[test]
    fn bundled_catalog_json_validates() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("model-catalog")
            .join("catalog.json");
        let text = std::fs::read_to_string(path).unwrap();
        let catalog: Catalog = serde_json::from_str(&text).unwrap();
        validate(&catalog).unwrap();
    }

    #[test]
    fn read_cache_propagates_parse_error_on_garbage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "{ not json").unwrap();
        let err = read_cache(&path).unwrap_err();
        assert!(matches!(err, CatalogError::Parse(_)));
    }
}
