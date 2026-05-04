//! Regenerates `src/lib/tauri/bindings.ts` from the shared `tauri_specta`
//! builder. Lives in `src/bin/` (not `examples/` or `tests/`) because on
//! Windows those targets fail to launch with `STATUS_ENTRYPOINT_NOT_FOUND`
//! in this crate's cdylib setup, while `src/bin/` binaries link the same
//! way as `src/main.rs` and run cleanly.
//!
//! Local: `cargo run --locked --bin export_bindings` — writes the file.
//! CI: same invocation, followed by `git diff --exit-code ../src/lib/tauri/bindings.ts`.

use specta_typescript::Typescript;

const BINDINGS_PATH: &str = "../src/lib/tauri/bindings.ts";

fn main() {
    abraxas_lib::__specta_builder()
        .export(
            Typescript::default().header("// @ts-nocheck\n"),
            BINDINGS_PATH,
        )
        .expect("failed to export specta bindings");

    normalize_bindings_file(BINDINGS_PATH).expect("failed to normalize specta bindings");

    println!("bindings written to src/lib/tauri/bindings.ts");
}

fn normalize_bindings_file(path: &str) -> std::io::Result<()> {
    let contents = std::fs::read_to_string(path)?;
    let normalized = normalize_generated_typescript(&contents);

    if normalized != contents {
        std::fs::write(path, normalized)?;
    }

    Ok(())
}

fn normalize_generated_typescript(source: &str) -> String {
    let mut normalized = String::with_capacity(source.len());

    for line in source.split_inclusive('\n') {
        let line_without_lf = line.strip_suffix('\n').unwrap_or(line);
        let line_content = line_without_lf
            .strip_suffix('\r')
            .unwrap_or(line_without_lf);

        push_normalized_line(&mut normalized, line_content.trim_end_matches([' ', '\t']));

        if line.ends_with('\n') {
            normalized.push('\n');
        }
    }

    normalized
}

fn push_normalized_line(normalized: &mut String, line: &str) {
    let mut rest = line;

    loop {
        if let Some(after_tab) = rest.strip_prefix('\t') {
            normalized.push('\t');
            rest = after_tab;
        } else if let Some(after_spaces) = rest.strip_prefix("    ") {
            normalized.push('\t');
            rest = after_spaces;
        } else {
            break;
        }
    }

    normalized.push_str(rest);
}

#[cfg(test)]
mod tests {
    use super::normalize_generated_typescript;

    #[test]
    fn strips_trailing_whitespace_and_normalizes_crlf() {
        let source = "const value = 1;  \r\n * \nlet next = true;\t";

        assert_eq!(
            normalize_generated_typescript(source),
            "const value = 1;\n *\nlet next = true;"
        );
    }

    #[test]
    fn normalizes_leading_four_space_indents_to_tabs() {
        let source = "commands: {\n    max_completion_tokens: number | null,\n}";

        assert_eq!(
            normalize_generated_typescript(source),
            "commands: {\n\tmax_completion_tokens: number | null,\n}"
        );
    }
}
