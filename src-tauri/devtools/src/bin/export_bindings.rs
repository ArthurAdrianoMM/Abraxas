//! Regenerates `src/lib/tauri/bindings.ts` from the shared `tauri_specta`
//! builder.
//!
//! Local: `cargo run --locked -p abraxas-devtools --bin export_bindings`.
//! CI: same invocation, followed by `git diff --exit-code ../src/lib/tauri/bindings.ts`.

use std::path::{Path, PathBuf};

use specta_typescript::Typescript;

/// Resolved from `CARGO_MANIFEST_DIR` (`<repo>/src-tauri/devtools`) instead of
/// the process CWD, so the binary writes the same file whether it is invoked
/// from the repo root, from `src-tauri/`, or by an editor.
fn bindings_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/lib/tauri/bindings.ts")
}

fn main() {
    let path = bindings_path();

    abraxas_lib::__specta_builder()
        .export(Typescript::default().header("// @ts-nocheck\n"), &path)
        .expect("failed to export specta bindings");

    normalize_bindings_file(&path).expect("failed to normalize specta bindings");

    println!("bindings written to src/lib/tauri/bindings.ts");
}

fn normalize_bindings_file(path: &Path) -> std::io::Result<()> {
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
