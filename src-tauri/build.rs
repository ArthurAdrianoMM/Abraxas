use std::path::{Path, PathBuf};

fn main() {
    stage_ggml_runtime();
    tauri_build::build()
}

/// Copy the ggml runtime out of `llama-cpp-sys-2`'s `OUT_DIR` into a stable
/// directory that `tauri.{linux,windows}.conf.json` ships as bundle resources.
///
/// Two kinds of file land here, and they are found by different mechanisms at
/// runtime (ADR 0001 §5.3.1):
///
///   - the backend modules ggml `dlopen`s from a directory we hand it
///     (`ggml-cuda`, `ggml-vulkan`, one `ggml-cpu-*` per CPU feature level);
///   - the llama/ggml shared libraries the OS loader resolves at process
///     start, because `dynamic-backends` implies `BUILD_SHARED_LIBS=ON`.
///
/// No-op on a static build — macOS, and any build without `dynamic-backends`.
/// `DEP_LLAMA_BACKENDS_DIR` only exists when that feature is on, which is also
/// why `llama-cpp-sys-2` is a direct dependency: Cargo passes `DEP_*` to the
/// build script of a crate that depends on the `links` crate itself, not to
/// one that reaches it through `llama-cpp-2`.
fn stage_ggml_runtime() {
    println!("cargo:rerun-if-env-changed=DEP_LLAMA_BACKENDS_DIR");
    let Ok(backends) = std::env::var("DEP_LLAMA_BACKENDS_DIR") else {
        return;
    };

    let backends = PathBuf::from(backends);
    // `$OUT_DIR/backends` -> `$OUT_DIR`, whose `lib/` (`bin/` on Windows) is
    // where the CMake install step puts the shared libraries.
    let out_dir = backends
        .parent()
        .expect("DEP_LLAMA_BACKENDS_DIR should have a parent");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let shared_libs = out_dir.join(if windows { "bin" } else { "lib" });

    let dest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .join("ggml-runtime");
    // Rebuilt from scratch: a module staged by an earlier feature set would
    // otherwise stay in the bundle and get loaded.
    if let Err(e) = std::fs::remove_dir_all(&dest) {
        assert!(
            e.kind() == std::io::ErrorKind::NotFound,
            "failed to clear {}: {e}",
            dest.display()
        );
    }
    std::fs::create_dir_all(&dest).unwrap_or_else(|e| panic!("create {}: {e}", dest.display()));

    // The backend modules have to keep the exact unversioned name, because
    // ggml's loader compares the file extension to `.so`/`.dll` verbatim
    // (`ggml/src/ggml-backend-reg.cpp`). Copying that name also dereferences
    // CMake's version symlinks, so one real file ships instead of three names
    // for the same content — which matters when one of them is ggml-cuda.
    let modules = stage(&backends, &dest, &|name| {
        name.ends_with(if windows { ".dll" } else { ".so" })
    });
    // The executable records each shared library by its SONAME (`libggml.so.0`),
    // so that is the name the loader asks for; the fully-versioned file and the
    // bare development symlink are both dead weight in a bundle. Windows has no
    // such versioning.
    let libs = stage(&shared_libs, &dest, &|name| {
        // CMake builds llama's `common` helper library whether or not the Cargo
        // feature that links it is on, and it is 6 MB. Nothing in this crate
        // links it, so shipping it would be dead weight in every installer.
        if name.starts_with("libllama-common") || name.starts_with("llama-common") {
            return false;
        }
        if windows {
            name.ends_with(".dll")
        } else {
            is_soname(name)
        }
    });

    assert!(
        modules > 0 && libs > 0,
        "staged {modules} backend modules and {libs} shared libraries from {} — \
         expected both to be non-empty for a dynamic-backends build",
        out_dir.display()
    );

    if !windows {
        set_origin_rpath(&dest);
        remove_broken_lib_symlinks();
    }
}

/// Remove the broken library symlinks `llama-cpp-sys-2` leaves in
/// `target/<profile>` and its `deps/` and `examples/` subdirectories, so that
/// its *next* run recreates them instead of panicking.
///
/// Upstream hard-links every `*.so` it finds beside the libraries it built into
/// those two directories, so `cargo run` and `cargo test` can load them. On
/// Linux the only `*.so` matches are CMake's bare development symlinks
/// (`libggml.so` -> `libggml.so.0`); `link(2)` does not follow symlinks, and the
/// SONAME file they point at is not `*.so`, so it is never copied alongside.
/// The result dangles. `Path::exists` follows symlinks and reports false for a
/// dangling one, so upstream's `if !dst.exists()` guard tries the link again and
/// dies with `AlreadyExists`.
///
/// That fires on the second build-script run in one target directory — which is
/// precisely `cargo clippy` followed by `cargo test`, i.e. CI. Our build script
/// runs after theirs, so cleaning up here is what unbreaks the next invocation.
/// Nothing is lost: a dangling symlink resolves to nothing, and the real
/// libraries are in `target/<profile>`, which cargo already puts on the loader
/// path.
///
/// Upstream bug, not ours, but `BUILD_SHARED_LIBS=ON` is what exposes it and
/// this crate is what turns that on.
fn remove_broken_lib_symlinks() {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    // `target/<profile>/build/<pkg>-<hash>/out` -> `target/<profile>`
    let Some(profile_dir) = Path::new(&out_dir).ancestors().nth(3) else {
        return;
    };
    // All three destinations upstream links into, and it panics on whichever
    // one it reaches first.
    let targets = [
        profile_dir.to_path_buf(),
        profile_dir.join("deps"),
        profile_dir.join("examples"),
    ];
    for dir in targets {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let is_symlink =
                std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink());
            let is_lib = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("lib") && n.ends_with(".so"));
            if is_symlink && is_lib && !path.exists() {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}

/// Give every staged library an `$ORIGIN` rpath: "look next to me".
///
/// Needed because linuxdeploy walks every ELF file in the AppDir and resolves
/// each one's `DT_NEEDED` entries against the system search path. It resolves
/// the executable's, since `.cargo/config.toml` gives the executable an rpath —
/// but the staged libraries have none, so `libggml-cpu-*.so` asking for
/// `libggml-base.so.0` is unresolvable even though the two files are in the
/// same directory, and the AppImage build dies with "Could not find
/// dependency: libggml-base.so.0".
///
/// It is also simply true: a module that can find its own siblings does not
/// depend on the process having already loaded them, which is what makes it
/// work today.
///
/// `patchelf` is not required to build the app — without it the `.deb`, and
/// every `cargo build`, are unaffected and only AppImage bundling fails, which
/// it does loudly. So a missing `patchelf` is a warning, not an error.
fn set_origin_rpath(dir: &Path) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
            .path();
        let output = std::process::Command::new("patchelf")
            // `--force-rpath` writes the legacy `DT_RPATH` instead of
            // `DT_RUNPATH`. The loader honours either, but linuxdeploy's own
            // ELF resolver reads only `DT_RPATH`, and it is linuxdeploy we are
            // doing this for.
            .arg("--force-rpath")
            .arg("--set-rpath")
            .arg("$ORIGIN")
            .arg(&path)
            .output();
        match output {
            Ok(out) if out.status.success() => {}
            Ok(out) => panic!(
                "patchelf failed on {}: {}",
                path.display(),
                String::from_utf8_lossy(&out.stderr).trim()
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                println!(
                    "cargo:warning=patchelf not found, so the staged ggml libraries carry no \
                     $ORIGIN rpath. `cargo build` and the .deb are fine; AppImage bundling will \
                     fail in linuxdeploy with \"Could not find dependency: libggml-base.so.0\". \
                     Install patchelf to bundle an AppImage."
                );
                return;
            }
            Err(e) => panic!("running patchelf on {}: {e}", path.display()),
        }
    }
}

/// Copy every file in `from` whose name `keep` accepts into `to`, flattened.
fn stage(from: &Path, to: &Path, keep: &dyn Fn(&str) -> bool) -> usize {
    let entries =
        std::fs::read_dir(from).unwrap_or_else(|e| panic!("read {}: {e}", from.display()));
    let mut staged = 0;
    for entry in entries {
        let path = entry
            .unwrap_or_else(|e| panic!("read {}: {e}", from.display()))
            .path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !keep(name) {
            continue;
        }
        std::fs::copy(&path, to.join(name))
            .unwrap_or_else(|e| panic!("copy {} to {}: {e}", path.display(), to.display()));
        staged += 1;
    }
    staged
}

/// Whether `name` is a SONAME link such as `libggml.so.0` — a `.so.` followed
/// by nothing but digits. `libggml.so` and `libggml.so.0.9.4` are both false.
fn is_soname(name: &str) -> bool {
    match name.rsplit_once(".so.") {
        Some((_, version)) => !version.is_empty() && version.bytes().all(|b| b.is_ascii_digit()),
        None => false,
    }
}
