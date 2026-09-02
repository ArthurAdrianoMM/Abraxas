# ADR 0001 — Load ggml backends dynamically instead of static-linking them

**Status:** Accepted. §6's open questions resolved 2026-09-02; implementation not started.
**Supersedes:** the build strategy described in CLAUDE.md §2.4 ("Implicações de build")
**Audience:** the engineer/agent implementing this change

**Amended 2026-09-02.** §6 now records answers instead of questions. Three claims were
wrong and are corrected in place: `GGML_BACKEND_PATH` is not a search directory
(§5.3.3); `dynamic-backends` also makes llama and ggml *themselves* shared libraries
(§5.3.1), the larger half of the bundling problem, which the original draft did not
name; and ggml's backend scoring ranks CPU variants only, not GPU backends, so it does
**not** delete the arch-list coupling the way §4 claimed (§4.2). §7 is resequenced
around the discovery that a CPU-only build exercises every new mechanism, macOS is
explicitly out of scope (§4.1), and §7.0's benchmark is recorded as unmeasurable for now
rather than pending.

---

## 0. How to use this document

Sections 1–3 are the problem: what the build does today, why it hurts, and the
evidence. Section 4 is the decision and its scope. Sections 5–7 are the implementation
surface, the resolved questions it rests on, and the sequencing. Section 8 is how we
know it worked.

Section 6 was the most important part: a list of things to verify before writing code,
several of which were derived from `llama-cpp-sys-2`'s `main` branch rather than the
pinned version. That verification is done. Section 6 now records the answers with the
file and line they came from, and section 5 is corrected where an answer contradicted
it.

Citations of the form `ggml-backend-reg.cpp:562` are into the llama.cpp vendored inside
`llama-cpp-sys-2` **0.1.155**; `sampling.rs:535` and friends are that version's own
sources. Everything else is this repo.

---

## 1. Current state

### 1.1. How backends are compiled in

GPU backends are Cargo features that forward to `llama-cpp-2`
(`src-tauri/Cargo.toml:101-109`):

```toml
default = []
cuda   = ["llama-cpp-2/cuda"]
vulkan = ["llama-cpp-2/vulkan"]
metal  = ["llama-cpp-2/metal"]
```

`llama-cpp-2` is pinned at **0.1.145** with `default-features = false`
(`Cargo.toml:89-99`). Everything links **statically** into the single `abraxas`
executable.

Release builds enable a per-OS combo (`.github/workflows/release.yml:107-124`):

| OS | Args | Bundles |
|---|---|---|
| macOS (aarch64) | `--features metal` | `dmg` |
| Windows | `--features cuda,vulkan` | `nsis,msi` |
| Linux | `--features cuda,vulkan` | `deb,appimage` |

### 1.2. How the backend is chosen at runtime — and what actually chooses it

This is widely misread in the codebase's own comments, so state it plainly:

- `hardware::gpu::detect()` (`src/hardware/gpu/mod.rs:114`) probes NVML → Vulkan → None.
- `hardware::selector::select_backend()` (`src/hardware/selector.rs:43`) maps that to
  `InferenceBackend::{Metal, Cuda, Vulkan, Cpu}` plus a user-facing `reason` string.
- **That enum does not select a ggml backend.** `LlamaCppBackend`
  (`src/inference/llama_cpp.rs:74-104`) consumes it only as an `OffloadPolicy`,
  which ends up as a single `n_gpu_layers` integer at
  `llama_cpp.rs:258`. The actual device is picked by whatever ggml backend was
  compiled into the binary.

So today `selector` answers *"offload to GPU, and what do we tell the user?"* —
not *"which backend runs."* Keep this distinction in mind: under this ADR, ggml
takes over the second question explicitly, and `selector` keeps the first.

### 1.3. The arch-list coupling

`CMAKE_CUDA_ARCHITECTURES` is set in CI (`release.yml:217`):

```
61-real;75-real;86-real;89-real;120-real;90-virtual
```

and is **hand-mirrored** in Rust by `ComputeCapability::has_cuda_kernel()`
(`src/hardware/gpu/mod.rs:78-90`), whose own doc comment says:

> The two lists are a pair: changing the workflow without changing this function
> makes the app promise CUDA to a GPU that has no kernel to run, and ggml then
> dies with "no kernel image is available for execution on the device" — after
> the model is already loading, which reads as a crash.

A build-time CMake variable in YAML is load-bearing for runtime dispatch logic in
Rust, with no mechanism enforcing the invariant beyond unit tests that encode the
same hardcoded list a third time (`gpu/mod.rs:174-212`).

---

## 2. The problem

Static-linking every GPU backend into one executable is the single root cause of
essentially every Windows/Linux release problem. The costs:

**Build time.** `nvcc` instantiates every ggml-cuda kernel template once per SM
architecture — five real archs plus one virtual. The release job budgets
`timeout-minutes: 120` and the workflow's own comments cite ~90 min for nvcc
(`release.yml:20-22, 224-225`). This is paid **on every tag**, because the CUDA
objects are entangled with the app binary and the release job deliberately runs
without `Swatinem/rust-cache` (`release.yml:296-299`).

**Binary size.** The union of every kernel for every supported GPU ships to every
user, of which ~99% is dead weight on any given machine. The earlier full arch
list produced a **1.29 GB `abraxas.exe`**, at which point `makensis` failed with
`Internal compiler error #12345: error mmapping file` (`release.yml:184-188`).
The current list is a size-driven compromise, not a coverage decision.

**A cascade of platform workarounds**, all downstream of "one giant static
binary":

| Symptom | Workaround | Location |
|---|---|---|
| `vulkan-shaders-gen` ExternalProject races under MSBuild (MSB8066) | force `CMAKE_GENERATOR=Ninja` | `release.yml:262-267` |
| Ninja's try-compile blows MAX_PATH, `cl.exe` C1083 | `CARGO_TARGET_DIR=D:\ct` | `release.yml:269-281` |
| `.cu` objects not PIC, PIE link fails on `stderr` reloc | `CMAKE_POSITION_INDEPENDENT_CODE=ON` | `release.yml:209-214` |
| `makensis` mmap ICE at 1.29 GB | trim arch list | `release.yml:184-199` |
| AppImage inherits runner glibc | pin `ubuntu-22.04` | `release.yml:120-123` |
| linuxdeploy needs FUSE 2 | install `libfuse2` | `release.yml:145` |
| CUDA toolset unsupported on VS2026 | pin `windows-2022` | `release.yml:113-117` |

**Fragility, expressed as history.** `git log` shows a sustained run of commits
whose entire content is CI firefighting on this one axis: `4b1f5ab`, `256c926`,
`b4c019a`, `824f26c`, `83e3593`, `71a622a`, `db0a794`, `59dffb1`. Two releases
(v0.1.0, v0.1.1) shipped broken and a third (v0.1.2) failed at the AppImage step.

**Blast radius.** A GPU backend that cannot initialize on the user's device
aborts inside the process. `has_cuda_kernel()` was written to pre-empt one
instance of this by refusing to promise CUDA to uncovered hardware — a
workaround for the fact that a statically linked backend has no way to decline.
It does not actually achieve that; see §4.2.

### 2.1. Why macOS is unaffected

Not better engineering — a structurally different backend. ggml-metal ships
Metal shader source that the OS compiles at load time: no per-architecture
codegen, no SDK install, no build-time shader compilation, and the framework is
already present on every target machine. Combined with a single target triple, a
single GPU vendor and a stable minimum OS, `--features metal` costs roughly the
price of the Rust build. **The macOS path is not a model to copy; it is a path
that never had the problem.**

---

## 3. What was considered and rejected

- **Drop CUDA, ship Vulkan-only.** Removes nvcc, the arch list, both SDK
  installs, the PIC workaround and most of the size. Rejected as the *primary*
  fix because it is a scope reduction, not an architectural correction — the
  static-link problems would return the moment any second backend is wanted.
  Still worth measuring — §7.0 intended to and could not, for lack of NVIDIA
  hardware, so this stays a live follow-up (§9) that this ADR makes cheap rather
  than forecloses.
- **Optional downloadable CUDA pack.** Adds a second versioned distribution
  channel, an app↔pack ABI compatibility matrix, and an unsigned-DLL-downloaded-
  post-install pattern that SmartScreen and enterprise EDR routinely quarantine —
  compounding the fact that the app is already unsigned (CLAUDE.md §2.5).
  Rejected as premature: it optimizes a size problem that has not been measured.
  **Explicitly out of scope for this implementation.** Note that this ADR makes it
  a later packaging change rather than a redesign, which is precisely why it
  should not be done now.
- **Vendor llama.cpp's official prebuilt release binaries.** Would eliminate nvcc
  entirely. Rejected: `llama-cpp-2` binds a specific vendored llama.cpp revision,
  and an ABI mismatch between the bindings and a differently-built ggml is
  undefined behaviour rather than a build error.

---

## 4. Decision

**Build ggml backends as dynamically loaded modules (`GGML_BACKEND_DL`) and let
ggml select among them at runtime.** All backends ship inside the installer as
before; what changes is that they become separate loadable libraries next to the
executable rather than object code inside it.

This is the path llama.cpp upstream maintains, and the one Ollama and LM Studio
ship. The static fat-binary path is deprioritized upstream, which is why the
workarounds in §2 accumulate.

Four properties we are buying:

1. **The expensive artifact stops being welded to the app binary.** *This
   property originally claimed the arch-list coupling was deleted by ggml's
   scoring. It is not — see the correction in §4.2.*
2. **The expensive artifact becomes cacheable across releases.** Once
   `ggml-cuda.{dll,so}` is a standalone file, it can be built by an independent
   CI job keyed on `(llama.cpp revision + arch list)` and reused by every app
   release that changed neither. App code changes weekly; the arch list changes
   almost never. **This is the actual fix for the 90 minutes** — dynamic loading
   alone does not make nvcc faster, it makes nvcc's output reusable.
3. **Failures degrade instead of aborting.** A module that fails to load, or
   whose score is zero, is skipped and the next candidate is tried. For the CPU
   this is exactly the mechanism that picks the right instruction-set variant.
   For GPU backends it is weaker than §4.2 originally assumed.
4. **NSIS never sees a 1.29 GB executable.** The size moves out of the single
   file that `makensis` mmaps.

**Bonus, and it is a real one:** the `dynamic-backends` path also enables
`GGML_CPU_ALL_VARIANTS`, which builds several CPU backends (baseline, AVX2,
AVX-512, …) as separately scored modules. The correct CPU variant is then chosen
at runtime rather than being fixed at compile time — which is a better answer to
CLAUDE.md §2.1's AVX2/AVX-512 detection goal than anything currently shipped.
`raw-cpuid` remains useful for *displaying* CPU capability to the user; it stops
being needed to *decide* anything.

### 4.1. Scope: Windows and Linux only

macOS keeps the static `--features metal` build. §2.1 already explains why: it is not a
platform that has this problem. Going dynamic there would put nested unsigned `.so`
modules inside `Abraxas.app`, and on Apple Silicon a module must carry at least an
ad-hoc signature to be `dlopen`ed at all — the same failure class that shipped as "app
is damaged" and was fixed by `b093065`. That is a real regression on the one platform
that currently works, bought for a marginal gain: ggml does offer `apple_m1` /
`apple_m2_m3` / `apple_m4` CPU variants (`ggml/src/CMakeLists.txt:425-428`), but there
is one target triple, one GPU vendor, and no arch matrix to collapse.

So `dynamic-backends` goes on the
`cfg(any(target_os = "windows", target_os = "linux"))` dependency entry
(`Cargo.toml:95-99`) only, and the macOS entry (`Cargo.toml:89-90`) is left alone. Both
code paths must keep compiling, which the `#[cfg(feature = ...)]` gating in §5.2 already
implies. Revisiting macOS is a follow-up (§9), not a prerequisite.

### 4.2. Correction: ggml scores CPU variants, not GPU backends

Found while implementing, by reading the module-registration path rather than
trusting the shape of the API. `GGML_BACKEND_DL_SCORE_IMPL` — the macro that
exports the `ggml_backend_score` symbol a module is ranked by — is used **only**
by the CPU backends (`ggml/src/ggml-cpu/arch/*/cpu-feats.cpp`). CUDA and Vulkan
use `GGML_BACKEND_DL_IMPL` alone (`ggml-cuda.cu:5563`,
`ggml-vulkan.cpp:19647`) and export no score at all.

The loader handles that case explicitly: when no candidate scores above zero it
falls back to loading `libggml-<name>.so` by its exact unversioned name
(`ggml-backend-reg.cpp:485-560`). So a GPU module present on disk is loaded
**unconditionally**, and `ggml_cuda_init` then registers every device
`cudaGetDeviceCount` reports, with no compute-capability filter
(`ggml-cuda.cu:217-246`). An sm_80 GPU still gets CUDA devices registered from a
build carrying no sm_80 cubin, and still dies at kernel launch.

Two consequences:

- **`ComputeCapability::has_cuda_kernel()` cannot be deleted on the grounds
  §4 gave**, and §5.2's instruction to delete it is withdrawn. What it *can* be
  is corrected: its doc comment claims it prevents the "no kernel image" crash,
  and that was never true. `select_backend`'s enum only picks an `OffloadPolicy`
  (§1.2), and `Cuda` and `Vulkan` both map to the same `new_auto_gpu()`
  (`lib.rs:92-93`), so downgrading a GPU to `Vulkan` changes the text shown to
  the user and nothing else. ggml was always free to pick CUDA anyway.
- **The honest fix is device pinning, not scoring.** `LlamaModelParams::with_devices`
  plus `list_llama_ggml_backend_devices()` (§6.3) can hand llama.cpp an explicit
  device list, which is what would actually keep an uncovered GPU off CUDA. That
  is a behavioural change to model loading, it cannot be verified without the
  hardware, and it is now a follow-up (§9) rather than a side effect of this
  ADR.

Everything else in §4 stands. The build-time, binary-size and packaging wins are
unaffected: they follow from the modules being separate files, not from scoring.

---

## 5. Implementation surface

### 5.1. What `dynamic-backends` does

Verified against `llama-cpp-sys-2` **0.1.155**, `build.rs:1057-1078`:

- `dynamic-backends = ["dynamic-link"]`, so `BUILD_SHARED_LIBS=ON` is implied. It is not
  a second feature to remember, and it is not optional. §5.3.1 is what that costs.
- Sets `GGML_BACKEND_DL=ON` and `GGML_CPU_ALL_VARIANTS=ON`.
- Sets `GGML_BACKEND_DIR=$OUT_DIR/backends`, which makes CMake install the MODULE
  libraries there instead of `CMAKE_INSTALL_BINDIR` (`ggml/src/CMakeLists.txt:265-276`).
- Emits `cargo:backends_dir=$OUT_DIR/backends`, i.e. `DEP_LLAMA_BACKENDS_DIR` — but only
  for a build script whose crate depends **directly** on `llama-cpp-sys-2`. `abraxas`
  depends on `llama-cpp-2`, so it does not see that variable today. §5.3.2.

Two things the original draft of this section did not know:

- `GGML_BACKEND_DIR` is *also* compiled into ggml as a `PUBLIC` define
  (`ggml/src/CMakeLists.txt:256`), so the build machine's `$OUT_DIR/backends` becomes
  search path #1 inside the shipped binary. Harmless — a missing directory is skipped —
  but it means the default search list is never empty, and "backends loaded" in a log
  proves nothing about *which* directory they came from unless we choose the directory
  ourselves (§5.3.3).
- `GGML_CPU_ALL_VARIANTS` is a hard `FATAL_ERROR` without `GGML_BACKEND_DL`
  (`ggml/src/CMakeLists.txt:371-376`) and is arch-gated (x86, ARM including Apple,
  PowerPC, s390x, riscv64). On x86 it produces ~14 CPU modules — `x64`, `sse42`,
  `sandybridge`, `haswell`, `skylakex`, `icelake`, `zen4`, `alderlake`, … — a few fewer
  under MSVC. This is where CLAUDE.md §2.1's AVX2/AVX-512 goal is actually satisfied.

### 5.2. Rust changes

- **`Cargo.toml`**: `llama-cpp-2` 0.1.145 → **0.1.155**. 0.1.146 is the minimum with
  `dynamic-backends`, but there is no reason to adopt a five-month-old intermediate. Add
  `dynamic-backends` to the `cfg(any(target_os = "windows", target_os = "linux"))` entry
  (`Cargo.toml:95-99`) only, per §4.1; do not add `dynamic-link` separately (§5.1).
  `crate-type` at `Cargo.toml:22-26` needs no change (§6.4).

  The only breaking change across 0.1.145 → 0.1.155 in the API this repo uses is
  `LlamaSampler::penalties`, which gained a leading `n_vocab: i32` (`sampling.rs:535`).
  One call site (`src/inference/llama_cpp.rs:343`), taking `LlamaModel::n_vocab`;
  applied in step 1 of §7.
  Everything else the repo touches — `LlamaBackend::init`, `LlamaModel::{load_from_file,
  str_to_token, token_to_piece, new_context}`, `LlamaModelParams::with_n_gpu_layers`,
  `LlamaContextParams::with_n_ctx`, `LlamaBatch::new`, the other `LlamaSampler`
  constructors, and every error type re-exported by `src/inference/error.rs` — is
  signature-identical. (`openai.rs` was dropped and `speculative.rs` added upstream;
  neither is used here.)
- **`src/hardware/gpu/mod.rs`**: **this bullet's original instruction to delete
  `ComputeCapability::has_cuda_kernel()` is withdrawn — see §4.2.** ggml does not score
  GPU backends, so nothing takes over the job. What is wrong here is not the function
  but its documentation: the doc comment on line 57 and the module comment on lines
  14-17 both claim the function prevents the "no kernel image is available" crash, and
  it never did — `Cuda` and `Vulkan` both select the same `OffloadPolicy` (§1.2), so
  downgrading one to the other changes user-facing text only. Correct both comments to
  say that, and keep the function, the `detect()` filter and the two mirror tests until
  device pinning replaces them (§9).
- **`src/hardware/selector.rs`**: update the doc comment at lines 12-16, which claims
  `detect` pre-filters unrunnable CUDA GPUs. `select_backend`'s logic is unchanged — it
  remains the advisory/offload-policy decision described in §1.2. Do source `reason`
  from `list_llama_ggml_backend_devices()` (§6.3) rather than from our own probe, so the
  onboarding UI cannot disagree with the component that actually runs the model.
- **`src/inference/llama_cpp.rs`**: `ensure_backend()` (lines 36-46) must load the
  modules before `LlamaBackend::init()`. `llama-cpp-2` provides the safe wrapper, so no
  `llama-cpp-sys-2` FFI is needed:

  ```rust
  #[cfg(feature = "dynamic-backends")]
  llama_cpp_2::llama_backend::load_backends_from_path(dir);
  let initialized = Arc::new(LlamaBackend::init()?);
  tracing::info!(devices = ?llama_cpp_2::list_llama_ggml_backend_devices(), "ggml backends registered");
  ```

  The ordering is what makes this safe rather than merely working: `llama_backend_init`
  calls `ggml_backend_load_all()` only `if (!ggml_backend_reg_count())`
  (`llama.cpp/src/llama.cpp:131-133`). Loading from our directory first therefore
  *suppresses* the implicit scan — and if our directory is wrong or empty, the count
  stays zero and the implicit scan still runs. A free fallback, and not a silent one:
  the device log distinguishes the two cases.

### 5.3. Bundling and runtime discovery

#### 5.3.1. Two classes of artifact, not one

`BUILD_SHARED_LIBS=ON` (§5.1) means llama and ggml themselves stop being static
archives. The installer therefore has to carry two kinds of file, found by two different
mechanisms, failing in two different ways:

| | Files | Found by | Mechanism we must provide |
|---|---|---|---|
| **A** | `libllama`, `libggml`, `libggml-base` (`.so` / `.dll`) | the OS loader, at process start, from the executable's import table | rpath, or the DLL search path |
| **B** | `libggml-cuda`, `libggml-vulkan`, `libggml-cpu-haswell`, … | `dlopen`, from ggml's registry, during `LlamaBackend::init()` | a directory we pass explicitly (§5.3.3) |

Class B is what the rest of this ADR is about. **Class A is the bigger risk**, and it is
invisible to `cargo build` and `cargo test`: cargo puts the target directory on the
platform's dynamic-loader path when it runs binaries, and `llama-cpp-sys-2` hard-links
the shared libraries into that directory precisely so this works (`build.rs:1406+`). A
green `cargo test` says nothing about whether an *installed* app can find them.

The two classes also fail in opposite directions on AppImage: relocating class A and
rewriting its search path is linuxdeploy's entire job, and it will never discover class
B, because nothing in the ELF headers mentions a `dlopen`ed module. Expect either a
build that launches and quietly reports CPU-only inference, or one that cannot start at
all — different bugs, different halves.

Each backend module links `ggml-base` itself (`ggml/src/CMakeLists.txt:283`). That
resolves without extra work, because the process has already loaded class A by the time
ggml `dlopen`s a module — but it does mean class A must be right before class B can work
at all. Fix them in that order.

#### 5.3.2. Getting the files out of `$OUT_DIR`

`DEP_LLAMA_BACKENDS_DIR` reaches only a build script whose crate depends *directly* on
`llama-cpp-sys-2` (Cargo's `links` / `DEP_*` rule). `abraxas` depends on `llama-cpp-2`.
Two options:

1. **Add a direct `llama-cpp-sys-2` dependency** at the matching version and copy both
   classes into one staged directory from `src-tauri/build.rs`. Deterministic, keyed to
   the build that actually happened, and identical for a local `cargo tauri build` and
   for CI. Cost: one more version to keep in step with `llama-cpp-2`.
2. **Glob `target/*/build/llama-cpp-sys-2-*/out/backends` from CI.** No manifest change,
   but then only CI can bundle the app, and the glob is ambiguous whenever two builds of
   the sys crate coexist in one target dir (different features, host vs target).

Prefer (1). Note that `llama-cpp-2`'s own `build.rs` already consumes the variable and
re-exports it as `llama_backend::BACKENDS_DIR`, so *application code* needs neither
option — only the file-copying step does.

#### 5.3.3. Runtime discovery

> **The original text of this section was wrong.** `GGML_BACKEND_PATH` is not a search
> directory. It is the path to a *single* library, handed to `ggml_backend_load()` after
> the normal search has finished, for loading an out-of-tree backend
> (`ggml-backend-reg.cpp:588-592`). Pointing it at a directory loads nothing.

The right API is `ggml_backend_load_all_from_path(dir)`, wrapped safely as
`llama_cpp_2::llama_backend::load_backends_from_path(&Path)`. Passing a directory
*replaces* the whole default search list — compiled-in `GGML_BACKEND_DIR`, then the
executable's directory, then the current working directory
(`ggml-backend-reg.cpp:485-495`) — with exactly one location that we chose and can log.

The shape of the change is otherwise the one this section originally recommended, and
for the reasons it gave: one code path instead of three platform-specific loader
configurations, and a search location that is explicit and loggable rather than implicit
in link flags. Only the call is different.

- Resolve the directory once during Tauri `setup()` (`src/lib.rs:29`) via
  `app.path().resource_dir()`, and stash it in a `OnceLock<PathBuf>` that
  `ensure_backend()` reads. `ensure_backend` is lazy and runs on a worker thread long
  after `setup`, so it cannot resolve an `AppHandle` for itself.
- Fall back to `llama_backend::BACKENDS_DIR` when that `OnceLock` is empty. This covers
  `cargo test`, `cargo run`, and the `llama_smoke` devtool, none of which have an
  `AppHandle`. Without it, every non-app entry point silently gets whatever the implicit
  scan happens to find, which is how a passing test comes to mean nothing.

#### 5.3.4. Per-OS work

- **Linux `.deb`**: Tauri installs the binary under `/usr/bin/` and bundle resources
  under `/usr/lib/<binary>/` — confirm the exact path rather than trusting this
  sentence. Class A then needs an rpath (`-C link-arg=-Wl,-rpath,$ORIGIN/../lib/abraxas`
  or equivalent), because cargo emits none. Decide one location and use it for both
  classes.
- **AppImage**: see §5.3.1. Verify by extracting the AppImage, not by launching it on a
  machine that happens to have the libraries somewhere else. This is the step that broke
  v0.1.2.
- **Windows**: class A must sit next to `abraxas.exe`. The imports are resolved before
  any of our code runs, so `AddDllDirectory`/`SetDllDirectory` is too late for it. Class
  B can live in a subdirectory, because §5.3.3 hands ggml an explicit path.
- **`.deb` CI guard**: `ci.yml:519-568` asserts the package ships *only* the app binary.
  That assertion inverts under this ADR: it must assert both classes are present, while
  still asserting the devtools are not.
- **macOS**: nothing. §4.1.

#### 5.3.5. A third failure class: what `BUILD_SHARED_LIBS` exposes upstream

Found by running the build, not by reading it. `llama-cpp-sys-2`'s build script
hard-links every `*.so` it finds beside the libraries it built into
`target/<profile>`, `target/<profile>/deps` and `target/<profile>/examples`, so
that `cargo run` and `cargo test` can load them (`build.rs:1405-1435`). Under
`BUILD_SHARED_LIBS=ON` that code path becomes live for the first time, and on
Linux it is broken:

- the `*.so` glob matches only CMake's bare development symlinks
  (`libggml.so` -> `libggml.so.0`), never the SONAME file itself;
- `link(2)` does not follow symlinks, so what gets hard-linked is the *symlink*;
- the file it points at is not `*.so`, so it is never copied alongside.

The result dangles. `Path::exists` follows symlinks and reports false for a
dangling one, so the guard `if !dst.exists() { hard_link(..).unwrap() }` tries
the link again and panics with `AlreadyExists`.

It fires on the **second** build-script run in one target directory — which is
`cargo clippy` followed by `cargo test`, i.e. the order `ci.yml`'s `rust` job
already uses. So this is not a packaging problem at all: it breaks an ordinary
Linux build, on all three destination directories, and it would have broken CI
on the first push regardless of how the bundling was done.

`src-tauri/build.rs` deletes those dangling `lib*.so` symlinks after staging.
Our build script runs after the sys crate's, so the cleanup is what unbreaks the
*next* invocation; nothing is lost, because a dangling symlink resolves to
nothing and the real libraries sit in `target/<profile>` where cargo already
points the loader. Worth reporting upstream — the bug is theirs, but
`dynamic-backends` is what makes it reachable, and it is reachable by anyone who
enables `dynamic-link` for any reason.

### 5.4. CI restructuring

This is where the build-time win is realized, and it is only possible **after**
§5.2/§5.3 land.

- Split backend compilation into its own job producing uploaded artifacts, keyed
  on `(llama.cpp revision, CMAKE_CUDA_ARCHITECTURES, SDK versions)` — deliberately
  **not** on `Cargo.lock` or app source.
- The release job downloads those artifacts and bundles them, rather than
  compiling them.
- Once CUDA is no longer linked into the app binary, re-test whether the
  `CMAKE_POSITION_INDEPENDENT_CODE` and MAX_PATH workarounds are still required.
  They may be; do not remove them speculatively, remove them with a passing build.
- There is currently **no Windows bundling job at all**: `ci.yml`'s `bundle` (line 430)
  is Linux-only. v0.1.1 and v0.1.2 both died in packaging, and Windows packaging is the
  half that has never been exercised outside a tag. Adding it belongs to this work, not
  to a follow-up.

### 5.5. Unrelated but adjacent — fix while bundling is open

`tauri.conf.json:53-58` declares only `libwebkit2gtk-4.1-0` and `libgtk-3-0` as
`.deb` dependencies. If the binary carries a dynamic dependency on
`libvulkan.so.1` (and possibly `libcudart`), a user without those installed gets
a loader error before any of our code runs — a strong candidate for the reported
Linux breakage. **Run `ldd` on a built Linux binary and reconcile the full list
against `deb.depends`.** Track this separately; it is not caused by this ADR, but this
ADR touches the same files, and §5.3.1 changes the answer — class A adds `libllama`,
`libggml` and `libggml-base` to that same `ldd` output, resolved by the same loader, at
the same moment, with the same symptom when it fails.

---

## 6. Open questions — resolved 2026-09-02

Answered by reading the sources of `llama-cpp-2` / `llama-cpp-sys-2` 0.1.145, 0.1.146
and 0.1.155, plus the llama.cpp vendored in 0.1.155. Nothing below is inferred from
documentation or from `main`.

**6.1. Does `dynamic-backends` exist at a version we can adopt? — Yes, 0.1.146+.**
It first appears in **0.1.146** (2026-04-30), one release after the pin; the current
release is **0.1.155** (2026-08-31). Adopt 0.1.155. Reviewing the API this repo uses
across that range turns up exactly one breaking change — `LlamaSampler::penalties`
gaining a leading `n_vocab: i32`, details and call site in §5.2. The upgrade is not the
non-trivial prerequisite this question feared; it is a one-line fix plus a lockfile
bump.

**6.2. How are backend modules loaded at runtime? — By `llama_backend_init` itself,
unless we load them first.**
`llama_backend_init` calls `ggml_backend_load_all()` under
`if (!ggml_backend_reg_count())` (`llama.cpp/src/llama.cpp:131-133`), so
`LlamaBackend::init()` does load them — but from the default search list, which is not a
location we control (§5.1). `llama-cpp-2` re-exports the explicit form safely as
`llama_backend::load_backends_from_path`, alongside a `BACKENDS_DIR` const
(`llama_backend.rs:187-208`), both gated on the `dynamic-backends` feature. No
`llama-cpp-sys-2` FFI is required. This answer also invalidated §5.3's original
recommendation; see the correction at §5.3.3.

**6.3. Can the registered backends be enumerated after init? — Yes.**
`llama_cpp_2::list_llama_ggml_backend_devices() -> Vec<LlamaBackendDevice>`
(`lib.rs:474-553`) returns, per device: `index`, `name` (`"Vulkan0"`), `description`
(`"NVIDIA GeForce RTX 3080"`), `backend` (`"CUDA"`), `memory_total`, `memory_free`, and a
type (`Cpu` / `Gpu` / `IntegratedGpu` / `Accelerator`). That covers both uses this
question was asked for: an integration test asserting the expected modules loaded, and
onboarding text that cannot disagree with reality.

It is also a superset of what `nvml-wrapper` and `ash` currently report — it yields free
VRAM for AMD and Intel, which the `ash` probe does not
(`src/hardware/gpu/vulkan.rs`). Replacing our own probing with it is a real
simplification and a genuinely separate decision; §9.

**6.4. Does `BUILD_SHARED_LIBS=ON` conflict with the `staticlib` crate-type? — No.**
`staticlib` archives this crate's own Rust objects; native libraries are resolved at the
final link of the binary and the `cdylib`, so a shared llama/ggml is orthogonal to it.
Leave `Cargo.toml:22-26` alone. (Whether `staticlib` is needed at all is a separate,
pre-existing question — it is an iOS artifact — and not worth disturbing here.)

**6.5. What is the actual size and build-time delta? — Still unknown, and it can only be
measured by building it.**
Not a blocker, because §7's sequence builds both ways regardless. Record per-OS installer
size and release wall-clock at step 1 (static, upgraded) and again at step 4 (dynamic,
GPU), so the delta is attributable to this ADR rather than to the version bump.

---

## 7. Sequencing

**7.0. The CUDA-vs-Vulkan measurement cannot be made right now.** No NVIDIA hardware is
available to this project, so CLAUDE.md §3.2's "10-15%" stays unverified — and must stop
being cited as though it were measured. CUDA is kept, on the weaker but honest grounds
that it is the better-tested path on NVIDIA in llama.cpp and that this ADR removes most
of what made shipping it painful. §3's Vulkan-only option is therefore **undecided, not
rejected**: the moment someone can run a prefill/decode benchmark, decide it. Nothing
below depends on the answer.

Do record current per-OS installer sizes and release wall-clock now. That is free, and
it is what §6.5 needs.

**The structural point that reorders everything.** A CPU-only build exercises every
mechanism this ADR introduces — `GGML_BACKEND_DL`, `GGML_CPU_ALL_VARIANTS` *and*
`BUILD_SHARED_LIBS`, hence both classes of artifact in §5.3.1 — with no CUDA Toolkit, no
Vulkan SDK and no nvcc. All of the packaging risk can be retired inside the existing
30-minute `bundle` job (`ci.yml:430`), fully decoupled from the 90-minute one. That is
the opposite of the original instinct to switch a real GPU backend on first.

Each step is one PR.

1. **Upgrade to 0.1.155, still static.** Fix `penalties` (§5.2). Green on all three OS
   with the existing feature combos. No behaviour change, trivially revertable, and it
   isolates any upstream surprise from everything that follows.
2. **`dynamic-backends` on Linux, CPU-only.** The whole design lands here: the
   `OnceLock` + `load_backends_from_path` + device logging (§5.3.3), the direct
   `llama-cpp-sys-2` dependency and the staging copy (§5.3.2), `bundle.resources`
   wiring, and the rpath for class A. Extend the `bundle` job to assert both classes are
   present in the `.deb` *and* inside an extracted AppImage (§5.3.4). No GPU SDK
   involved, so iteration costs minutes.
3. **Windows, CPU-only.** Same design, different loader rules (§5.3.4). Add the Windows
   bundling job that does not exist yet, asserting NSIS/MSI contents the same way. Still
   no SDKs.
4. **Turn on `cuda,vulkan`.** Only now does nvcc enter. Correct the two comments that
   misdescribe `has_cuda_kernel()` (§5.2 — the function itself stays, per §4.2) and run
   §5.5's `ldd` reconciliation against `deb.depends`. Re-test whether the
   `CMAKE_POSITION_INDEPENDENT_CODE` and MAX_PATH workarounds are still required — with
   a passing build, not speculatively. Leave the arch list alone: with the modules out
   of the executable the NSIS size ceiling should be gone, but "should be" is not a
   measurement, and widening the list is a size/coverage decision to make against real
   numbers from step 5, not a drive-by edit.
5. **Per-backend artifact caching (§5.4).** The 90 minutes dies here, not earlier.
6. **macOS.** Out of scope per §4.1; listed so the sequence is honest about where it
   stops.

Steps 2 and 3 iterate locally against a Linux container where possible; only step 3's
packaging genuinely needs a Windows runner. The `bundle` smoke test is the safety net
throughout — it exists because v0.1.1 shipped broken, and it is extended *in* step 2,
not after.

---

## 8. Acceptance criteria

Scoped to Windows and Linux per §4.1. macOS keeps the criteria it already satisfies.

- A fresh clone still builds CPU-only with no GPU SDK installed (`cargo check` with
  `default = []` — CLAUDE.md §2.4).
- The installed app logs which ggml backends registered and which device was selected,
  sourced from `list_llama_ggml_backend_devices()` (§6.3) rather than from our own probe,
  and that selection matches the hardware.
- ~~An NVIDIA GPU with no shipped cubin falls back to Vulkan without any Rust-side arch
  list.~~ **Withdrawn (§4.2):** ggml does not score GPU backends, so dynamic loading
  does not deliver this. It requires device pinning, which is a follow-up (§9). Until
  then the criterion is only that this ADR does not make the case *worse* — an
  uncovered GPU behaves exactly as it does today.
- ~~No `CMAKE_CUDA_ARCHITECTURES` value is referenced anywhere in `src/`.~~ **Withdrawn
  (§4.2)**, for the same reason: the mirror in `has_cuda_kernel()` stays until something
  actually replaces it. Its doc comment must stop claiming it prevents a crash.
- The `.deb`, AppImage and NSIS/MSI each contain **both** classes of artifact from
  §5.3.1, verified by CI — and for the AppImage by extracting it, not by launching it on
  a machine that may have the libraries elsewhere.
- A Windows bundling job exists in `ci.yml` and asserts installer contents. Its absence
  is why v0.1.1 and v0.1.2 shipped broken.
- Windows NSIS bundling succeeds, and the `abraxas.exe` it packages is a small fraction
  of the 1.29 GB that produced the `makensis` mmap ICE — the size now lives in separate
  module files. (Whether to then *widen* the arch list is a separate decision; §7 step 4.)
- A second release from an unchanged `llama.cpp` revision reuses cached backend artifacts
  and completes substantially faster than the current ~90 min.
- Linux launches on a clean machine with no manually installed SDKs. This now requires
  both §5.5 and class A being correct, and those are different fixes.
- The DMG still builds, installs and runs, with the static Metal build unchanged.

---

## 9. Follow-ups this ADR deliberately leaves open

- **Whether to keep CUDA at all.** §7.0 was supposed to decide it and could not: no
  NVIDIA hardware. Undecided, not decided.
- **Pinning devices explicitly so an uncovered GPU never gets CUDA** — the fix §4.2
  showed this ADR does not provide. `LlamaModelParams::with_devices` plus
  `list_llama_ggml_backend_devices()` make it possible for the first time; it changes
  model loading behaviour and cannot be verified without an NVIDIA GPU that the shipped
  cubins do not cover, which is why it is not folded in here. It is also what would
  finally let `has_cuda_kernel()` and its arch-list mirror be deleted.
- **Whether to make macOS dynamic** (§4.1). Small upside, real signing risk, no urgency.
- **Whether `list_llama_ggml_backend_devices()` should replace the `nvml-wrapper` and
  `ash` probes outright** (§6.3). It reports more than they do, from the component that
  actually runs the model, and would delete two dependencies plus
  `src/hardware/gpu/{nvml,vulkan}.rs`. It also changes what the app can say about
  hardware *before* a backend is initialized — which is a design question about
  onboarding, not a refactor, and deserves its own ADR rather than being smuggled into
  this one.
- **Whether to split backends into optional downloads** (§3) — a packaging change once
  this ADR lands, not a redesign.
- **Updating CLAUDE.md** §2.4 "Implicações de build" and §3.2 to describe the new
  strategy, and to stop presenting the unmeasured "10-15%" as a fact. Do this at the end,
  from what was actually built.
