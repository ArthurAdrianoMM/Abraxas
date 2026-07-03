//! App settings persistence (key/value store). Fase 6.2.
//!
//! Named `app_settings` (not `settings`) to avoid shadowing
//! `commands/settings.rs` in imports.
//!
//! Each `AppSettings` field is one row in `app_settings`, keyed by the field
//! name with a JSON-encoded value. Missing rows fall back to
//! `AppSettings::default()`, so a fresh database needs no seeding and new
//! fields need no migration.

use sqlx::SqlitePool;

use crate::chat::SamplingParams;

/// Completion budget applied when neither the conversation nor the caller
/// pins `max_completion_tokens`. Referenced by `commands/chat.rs` so the
/// generation fallback and the settings default never drift apart.
pub const DEFAULT_MAX_COMPLETION_TOKENS: u32 = 512;

/// Reading-size preset for chat prose. Purely presentational; the frontend
/// maps it onto a root data attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum FontSize {
    Compacta,
    Comoda,
    Ampla,
}

/// Result of the last "conferir integridade" run over installed models.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct IntegrityCheck {
    /// RFC3339 timestamp of when the check finished.
    pub at: String,
    /// Model ids whose file hash no longer matches the registry (or whose
    /// file is missing). Empty = everything intact.
    pub corrupt: Vec<String>,
}

/// The full set of app-wide preferences. Defaults must match what the app
/// already does when no setting exists (see `SamplingParams::default()` and
/// `DEFAULT_MAX_COMPLETION_TOKENS`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct AppSettings {
    pub font_size: FontSize,
    /// Model auto-loaded on startup. `None` = first installed model.
    pub default_model_id: Option<String>,
    /// Stamped onto new conversations at creation time (existing
    /// conversations keep whatever they already have).
    pub default_temperature: f64,
    pub default_top_p: f64,
    pub default_max_completion_tokens: u32,
    /// `None` = random seed per conversation (NULL column, llama.cpp default).
    pub default_seed: Option<i64>,
    pub last_integrity_check: Option<IntegrityCheck>,
}

impl Default for AppSettings {
    fn default() -> Self {
        let sampling = SamplingParams::default();
        Self {
            font_size: FontSize::Comoda,
            default_model_id: None,
            default_temperature: sampling.temperature as f64,
            default_top_p: sampling.top_p as f64,
            default_max_completion_tokens: DEFAULT_MAX_COMPLETION_TOKENS,
            default_seed: None,
            last_integrity_check: None,
        }
    }
}

/// Loads settings by overlaying stored rows onto the defaults. Rows with
/// unknown keys (from newer app versions) are ignored; rows whose value no
/// longer parses keep that field's default.
pub async fn get(pool: &SqlitePool) -> Result<AppSettings, sqlx::Error> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM app_settings")
        .fetch_all(pool)
        .await?;

    let mut merged = match serde_json::to_value(AppSettings::default()) {
        Ok(serde_json::Value::Object(map)) => map,
        _ => return Ok(AppSettings::default()),
    };

    for (key, raw) in rows {
        if !merged.contains_key(&key) {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
            tracing::warn!(%key, "app_settings: value is not valid JSON, using default");
            continue;
        };
        let mut candidate = merged.clone();
        candidate.insert(key.clone(), value);
        if serde_json::from_value::<AppSettings>(serde_json::Value::Object(candidate.clone()))
            .is_ok()
        {
            merged = candidate;
        } else {
            tracing::warn!(%key, "app_settings: value no longer fits its field, using default");
        }
    }

    Ok(serde_json::from_value(serde_json::Value::Object(merged)).unwrap_or_default())
}

/// Upserts every field as its own row.
pub async fn set(pool: &SqlitePool, settings: &AppSettings) -> Result<(), sqlx::Error> {
    let object = match serde_json::to_value(settings) {
        Ok(serde_json::Value::Object(map)) => map,
        _ => return Ok(()),
    };
    for (key, value) in object {
        sqlx::query(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(&key)
        .bind(value.to_string())
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Removes every settings row ("queimar tudo" resets preferences too).
pub async fn clear(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM app_settings").execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
    use std::str::FromStr;

    async fn test_pool() -> SqlitePool {
        let pool =
            SqlitePool::connect_with(SqliteConnectOptions::from_str("sqlite::memory:").unwrap())
                .await
                .unwrap();
        sqlx::query(
            "CREATE TABLE app_settings (
                key   TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn empty_table_returns_defaults() {
        let pool = test_pool().await;
        let settings = get(&pool).await.unwrap();
        assert_eq!(settings, AppSettings::default());
        assert_eq!(settings.font_size, FontSize::Comoda);
        assert!(settings.default_model_id.is_none());
    }

    #[tokio::test]
    async fn set_then_get_round_trips() {
        let pool = test_pool().await;
        let settings = AppSettings {
            font_size: FontSize::Ampla,
            default_model_id: Some("tinyllama-1.1b".into()),
            default_temperature: 1.2,
            default_top_p: 0.5,
            default_max_completion_tokens: 2048,
            default_seed: Some(365),
            last_integrity_check: Some(IntegrityCheck {
                at: "2026-07-03T00:00:00Z".into(),
                corrupt: vec!["broken-model".into()],
            }),
        };
        set(&pool, &settings).await.unwrap();
        assert_eq!(get(&pool).await.unwrap(), settings);
    }

    #[tokio::test]
    async fn set_is_an_upsert() {
        let pool = test_pool().await;
        let mut settings = AppSettings::default();
        set(&pool, &settings).await.unwrap();
        settings.default_temperature = 0.2;
        set(&pool, &settings).await.unwrap();
        assert_eq!(get(&pool).await.unwrap().default_temperature, 0.2);
    }

    #[tokio::test]
    async fn unknown_keys_and_bad_values_fall_back_to_defaults() {
        let pool = test_pool().await;
        for (key, value) in [
            ("from_the_future", "\"whatever\""),
            ("font_size", "\"gigante\""),
            ("default_seed", "42"),
        ] {
            sqlx::query("INSERT INTO app_settings (key, value) VALUES (?1, ?2)")
                .bind(key)
                .bind(value)
                .execute(&pool)
                .await
                .unwrap();
        }
        let settings = get(&pool).await.unwrap();
        // Unknown key ignored, invalid enum variant falls back, valid field kept.
        assert_eq!(settings.font_size, FontSize::Comoda);
        assert_eq!(settings.default_seed, Some(42));
    }

    #[tokio::test]
    async fn clear_wipes_all_rows() {
        let pool = test_pool().await;
        let settings = AppSettings {
            default_temperature: 1.5,
            ..AppSettings::default()
        };
        set(&pool, &settings).await.unwrap();
        clear(&pool).await.unwrap();
        assert_eq!(get(&pool).await.unwrap(), AppSettings::default());
    }
}
