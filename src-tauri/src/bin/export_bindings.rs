//! Regenerates `src/lib/tauri/bindings.ts` from the shared `tauri_specta`
//! builder. Lives in `src/bin/` (not `examples/` or `tests/`) because on
//! Windows those targets fail to launch with `STATUS_ENTRYPOINT_NOT_FOUND`
//! in this crate's cdylib setup, while `src/bin/` binaries link the same
//! way as `src/main.rs` and run cleanly.
//!
//! Local: `cargo run --locked --bin export_bindings` — writes the file.
//! CI: same invocation, followed by `git diff --exit-code ../src/lib/tauri/bindings.ts`.

use specta_typescript::Typescript;

fn main() {
    abraxas_lib::__specta_builder()
        .export(
            Typescript::default().header("// @ts-nocheck\n"),
            "../src/lib/tauri/bindings.ts",
        )
        .expect("failed to export specta bindings");

    println!("bindings written to src/lib/tauri/bindings.ts");
}
