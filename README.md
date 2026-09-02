# Abraxas

Local-first desktop chat app for open-source LLMs. Cross-platform (Windows, macOS, Linux), built with Tauri v2 + Rust + React. All inference runs on your machine — no cloud, no API keys, no telemetry.

See [CLAUDE.md](CLAUDE.md) for the full project context: vision, stack, architecture decisions, roadmap.

**Status:** Fase 3.5 — token streaming + cancellation in a temporary dev screen. Not yet usable as an end-user app.

## Building from source

GPU backends are opt-in Cargo features. Pick the one(s) your hardware supports — you don't need every SDK installed locally.

| You have... | Build command | What you need installed |
|---|---|---|
| NVIDIA GPU | `cargo build --features cuda` | [CUDA Toolkit](https://developer.nvidia.com/cuda-downloads) (sets `CUDA_PATH`) |
| AMD / Intel GPU | `cargo build --features vulkan` | [Vulkan SDK](https://www.lunarg.com/vulkan-sdk/) (sets `VULKAN_SDK`) |
| Apple Silicon | `cargo build --features metal` | Xcode (Metal frameworks ship with it) |
| CPU only / fresh clone smoke test | `cargo build` | Nothing — CPU fallback works without SDKs |
| Production-equivalent (Win/Linux) | `cargo build --features cuda,vulkan` | Both CUDA Toolkit and Vulkan SDK |

Run all `cargo` commands from the `src-tauri/` directory, or pass `--manifest-path src-tauri/Cargo.toml`.

After installing an SDK on Windows, **restart your shell** so the env var (`CUDA_PATH` / `VULKAN_SDK`) propagates.

The shipped installers (downloaded from GitHub Releases) always include the full per-OS combo — end users never need to install anything.

### Frontend

```bash
pnpm install
pnpm tauri dev          # full app (requires the Rust feature flags above)
pnpm exec tsc --noEmit  # frontend type check (no Rust build)
```

### Regenerating typed bindings

After changing any `#[tauri::command]` or `#[derive(Event)]`:

```bash
cargo run --locked -p abraxas-devtools --bin export_bindings
```

This rewrites `src/lib/tauri/bindings.ts`. CI fails if the file drifts from the regenerated output.

### Dev tools

`src-tauri/devtools/` is a separate workspace member holding the binaries that
are useful during development but must never ship:

| Binary | What it does |
|---|---|
| `export_bindings` | Regenerates `src/lib/tauri/bindings.ts` (above) |
| `llama_smoke` | Loads a GGUF model and streams tokens to stdout, exercising the same hardware detection → backend selection path as the app |

```bash
cargo run --release -p abraxas-devtools --bin llama_smoke --features abraxas-devtools/metal -- \
  --model ~/models/tinyllama.gguf --prompt "hello"
```

The crate forwards `cuda` / `vulkan` / `metal` to the app crate, so pass whichever
combo you build the app with.

They live outside the `abraxas` package on purpose. The Tauri bundler copies
**every** `[[bin]]` target of the app package into the installer and ignores
`required-features`, so a helper binary declared there either bloats the bundle
(v0.1.0 shipped a 13 MB `export_bindings` inside `Abraxas.app`) or breaks
`cargo tauri build` outright (v0.1.1, which tried to gate them behind a feature
and died with `Failed to copy binary from .../export_bindings: does not exist`).
In a separate crate the bundler simply never sees them. CI enforces this: the
`abraxas` package must declare exactly one binary.

## Cutting a release

Tag it. That's the whole procedure:

```bash
git tag -a v0.1.4 -m "Abraxas v0.1.4"
git push origin v0.1.4
```

The tag is the only source of truth for the version. `src-tauri/Cargo.toml`
declares `version = "0.0.0"` in the repo; each release runner writes the real
version into it with `scripts/set-version.sh` right before the bundler reads it,
and nothing is committed. There is no bump commit to forget and no version guard
to abort the tag.

`tauri.conf.json` deliberately omits `version` — without the field the Tauri v2
bundler falls back to the Cargo.toml version, so one file declares it instead of
four. `package.json` omits it too: it's `private: true` and never published.

One consequence: a local `cargo tauri build` produces `Abraxas_0.0.0`, and the
settings screen (which reads `CARGO_PKG_VERSION`) shows `0.0.0`. Run
`./scripts/set-version.sh v0.1.4` first to reproduce a release build locally —
just don't commit the result.

Pushing the `vX.Y.Z` tag builds the three installers and attaches them to a
**draft** GitHub Release — nothing is published until the draft is reviewed and
released by hand. A tag with a suffix (`v0.1.1-rc.1`) is marked pre-release and
exercises the whole pipeline without burning the final version; the suffix exists
only in the tag, never in the manifests.

If a tag was pushed at the wrong commit, delete it locally and remotely (and
delete the draft release it created) before re-tagging:

```bash
git tag -d v0.1.1 && git push origin :refs/tags/v0.1.1
gh release delete v0.1.1 --yes   # only if a draft was created
```

## License

MIT — see [LICENSE](LICENSE).
