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
cargo run --locked --bin export_bindings --features dev-bins,cuda,vulkan
# or whichever GPU feature combo you've been building with
```

This rewrites `src/lib/tauri/bindings.ts`. CI fails if the file drifts from the regenerated output.

The `dev-bins` feature is required: `export_bindings` and `llama_smoke` are gated
behind it so that `cargo tauri build` compiles only the `abraxas` binary. Without
the gate every binary in the target dir gets copied into the installer, and with
`cuda,vulkan` each one carries its own copy of llama.cpp.

## Cutting a release

The version lives in four files — `package.json`, `src-tauri/tauri.conf.json`,
`src-tauri/Cargo.toml` and `src-tauri/Cargo.lock` — and the release workflow's
version guard aborts the tag if any of them disagrees with it. Don't edit them by
hand; bump all four, commit and tag in one step:

```bash
./scripts/bump-version.sh 0.1.1
git push origin main --follow-tags
```

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
