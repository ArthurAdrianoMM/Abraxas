# Abraxas Settings Phase — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Real settings persistence (backend + Settings view per `docs/design/Abraxas Settings.html`) plus Phase-2 manager/download retrofits (disk usage, tornar padrão, abrir pasta).

**Architecture:** Key/value `app_settings` SQLite table serialized into one typed `AppSettings` struct; two commands (get/set full struct, read-modify-write from a zustand settings store). Additive commands: `disk_usage`, `verify_installed_models`, `clear_conversations`, `clear_all_data`. Defaults stamped onto conversations at creation time.

**Tech Stack:** sqlx (dynamic queries), specta bindings via `cargo run --bin export_bindings`, sysinfo::Disks, @tauri-apps/plugin-opener, zustand + CSS Modules.

---

## Design deviations (locked)

1. **Theme radio omitted** — single crafted dark theme; no palettes exist for pergaminho/alta tinta/automático; dead controls forbidden.
2. **Models dir display-only** — changing dir requires folder picker (new dep `plugin-dialog`) + file migration; deferred. "abrir pasta" ships.
3. **Backend shown read-only** (not a `<select>`) — CLAUDE.md §2.4: backend selection is automatic, never user-chosen.
4. **Ornaments toggle** — only if chat renders ornaments; verify, else omit + log.
5. Default-model control lives in Manager ("tornar padrão"), not Settings — matches both designs.

## Settings schema (single source of truth)

Table (migration `0005_app_settings.sql`):
```sql
-- Fase 6.2: app-wide settings, key/value with JSON values.
CREATE TABLE IF NOT EXISTS app_settings (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
```

Typed struct (`db/app_settings.rs`), each field one row, JSON-encoded value; missing row = default:
```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct AppSettings {
    pub font_size: FontSize,                       // Compacta | Comoda | Ampla (default Comoda)
    pub default_model_id: Option<String>,
    pub default_temperature: f32,                  // default = SamplingParams::default().temperature
    pub default_top_p: f32,                        // default = SamplingParams::default().top_p
    pub default_max_completion_tokens: u32,        // default = DEFAULT_COMPLETION_BUDGET (512)
    pub default_seed: Option<i64>,                 // None = random
    pub last_integrity_check: Option<IntegrityCheck>, // { at: rfc3339, ok: bool, corrupt: Vec<String> }
}
```
Module fns: `get(pool) -> AppSettings` (fold rows over defaults), `set(pool, &AppSettings)` (upsert every key). In-file `#[cfg(test)]` round-trip + defaults-when-empty tests (in-memory pool, hand CREATE TABLE, per registry.rs pattern).

## Commands (all in `commands/settings.rs` unless noted)

- `get_app_settings(db) -> AppSettings`
- `set_app_settings(db, settings: AppSettings) -> AppSettings`
- `disk_usage(app) -> DiskUsage { models_dir: String, free_bytes: u64, total_bytes: u64 }` — sysinfo::Disks, longest mount-point prefix match against app_data_dir; spawn_blocking.
- `verify_installed_models(app, db) -> IntegrityReport { checked: u32, corrupt: Vec<String> }` — stream sha256 per installed file vs registry, persists `last_integrity_check`, missing file counts corrupt.
- `clear_conversations(db) -> ()` — DELETE messages, conversations.
- `clear_all_data(app, db, manager) -> ()` — unload model, delete model files on disk, DELETE installed_models + messages + conversations + app_settings rows.

Wiring: `mod settings;` in `commands/mod.rs`, add all to `collect_commands![]` in `specta.rs`, regenerate bindings.

Defaults-at-creation: `create_conversation` (commands/conversations.rs) reads `app_settings::get` and stamps temperature/top_p/max_completion_tokens/seed columns on the new row (top_k/repeat_* stay NULL). Old conversations untouched.

## Frontend

- `src/stores/settings.ts` — catalog.ts pattern (status + inFlight), `init()`, `save(patch: Partial<AppSettings>)` (merge → `setAppSettings` → set state), `setDefaultModel(id | null)`.
- `src/stores/model.ts` init: prefer `settings.default_model_id` if installed, else `installed[0]`.
- Font size: `document.documentElement.dataset.fontSize` set from store; `design-system.css` adds `[data-font-size="compacta|ampla"]` scaling for chat prose.
- `SettingsView.tsx` + `SettingsView.module.css` — full screen per design: shead, sections I–V (I aparência: letra chips [+ ornamentos if wired]; II pasta: path `ti` read-only + abrir pasta + `N modelos · X gb · disponível: Y gb` + conferir integridade + última; III sliders temp 0–2 / top-p 0–1 + max tokens + seed inputs; IV backend read-only + hw summary + `examinar de novo` → `detectHardware(force)` via hardware store; V sobre (appInfo/version) + apagar histórico + queimar tudo, inline confirm pattern from ManagerPane).
- Manager retrofits (`ManagerPane.tsx`): disk row gains `· X GB livres no disco` + `N% do disco` + meter (usedByModels/total); rows gain seal ★ when default, verbs `tornar padrão`/`já é padrão` (disabled) + `abrir pasta` (opener `openPath` on dir containing file) + existing remover.
- Download retrofit (`DownloadPane.tsx`): storage row gains meter (filled = used/total, incoming = model size), `data-warn` when `size + 1GB > free`, warn line per Error States copy.
- Opener capability: grant `opener:allow-open-path` (or reveal) in `src-tauri/capabilities/`.
- Clear-data flows refresh conversations store (sidebar/chat immediately) and, for queimar tudo, model + settings stores.

## Tasks

1. **feat(settings): db module + migration** — 0005 migration, `db/app_settings.rs` with tests → `cargo test`.
2. **feat(settings): commands + bindings** — get/set + wire specta; `disk_usage`; `verify_installed_models`; `clear_conversations`/`clear_all_data`; stamp defaults in `create_conversation`; export bindings; `cargo check` + `cargo test`.
3. **feat(settings): settings store + default model + font size** — store, model.ts init change, font-size root attr + CSS.
4. **feat(settings): Settings view** — full view per design.
5. **feat(models): manager retrofits** — disk row, tornar padrão, abrir pasta (+ capability).
6. **feat(models): download storage meter + warn.**
7. Verify end-to-end (npm run build, cargo check/test, live app exercise), visual compare, final summary.

Commit per task, conventional commits, no co-author trailers.
