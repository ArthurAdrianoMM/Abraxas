//! Metadata of a GGUF file, read from its header only.
//!
//! The catalog tells us everything about a catalog model. For a model the user
//! brought themselves — a file already on disk, or a URL they pasted — the
//! only source of truth is the file, and its GGUF header carries what the
//! app needs: architecture, a human name, the trained context length and
//! the chat template. ggml's gguf reader parses that header without
//! touching the tensors (`no_alloc`), so this costs milliseconds even for a
//! 40 GB file.
//!
//! This is also the sanity gate for user-supplied models, which are not
//! checksum-verified: a truncated download or a file that only has a `.gguf`
//! extension fails here with a clear error instead of a cryptic one from
//! llama.cpp at load time.

use std::ffi::{CStr, CString};
use std::path::Path;
use std::ptr::NonNull;

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

/// What the header of a GGUF tells about the model inside.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct GgufInfo {
    /// `general.architecture` — e.g. `llama`, `qwen2`, `gemma3`.
    pub architecture: String,
    /// `general.name`, when the converter recorded one.
    pub name: Option<String>,
    /// `general.size_label`, e.g. `8B` or `4x7B`.
    pub size_label: Option<String>,
    /// Quantization derived from `general.file_type`, e.g. `Q4_K_M`.
    pub quantization: Option<String>,
    /// `<architecture>.context_length`.
    pub context_length: Option<u32>,
    /// Whether llama.cpp can render the embedded `tokenizer.chat_template`.
    /// `false` also when there is no template at all — in either case the
    /// user has to pick a family by hand.
    pub embedded_template_supported: bool,
    /// Size of the file on disk.
    pub size_bytes: u64,
}

#[derive(Debug, Error)]
pub enum GgufError {
    #[error("file not found: {0}")]
    NotFound(String),
    #[error("not a valid GGUF file (bad magic, truncated, or unreadable header)")]
    Invalid,
    #[error("GGUF header has no general.architecture")]
    NoArchitecture,
    #[error("this file is shard {no} of {count}; split GGUFs are not supported — pick a single-file quantization")]
    Split { no: u32, count: u32 },
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Read the header of the GGUF at `path`.
pub fn inspect(path: &Path) -> Result<GgufInfo, GgufError> {
    let meta = std::fs::metadata(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => GgufError::NotFound(path.display().to_string()),
        _ => GgufError::Io(e),
    })?;
    if !meta.is_file() {
        return Err(GgufError::Invalid);
    }

    let ctx = Header::open(path).ok_or(GgufError::Invalid)?;

    let architecture = str_value(&ctx, "general.architecture")
        .ok_or(GgufError::NoArchitecture)?
        .to_owned();

    if let Some(count) = u32_value(&ctx, "split.count").filter(|c| *c > 1) {
        return Err(GgufError::Split {
            no: u32_value(&ctx, "split.no").unwrap_or(0) + 1,
            count,
        });
    }

    let template = chat_template(&ctx);

    Ok(GgufInfo {
        name: str_value(&ctx, "general.name")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
        size_label: str_value(&ctx, "general.size_label").map(str::to_owned),
        quantization: u32_value(&ctx, "general.file_type").and_then(quantization_name),
        context_length: u32_value(&ctx, &format!("{architecture}.context_length")),
        embedded_template_supported: template
            .as_deref()
            .map(crate::chat::embedded::supports)
            .unwrap_or(false),
        architecture,
        size_bytes: meta.len(),
    })
}

/// The raw `tokenizer.chat_template` of the GGUF at `path`, if any. Used at
/// load time to hand the template to the renderer.
pub fn embedded_chat_template(path: &Path) -> Result<Option<String>, GgufError> {
    let ctx = Header::open(path).ok_or(GgufError::Invalid)?;
    Ok(chat_template(&ctx))
}

/// Header-only handle on a GGUF. A thin wrapper over the ggml C API rather
/// than `llama_cpp_2::gguf::GgufContext` because that one exposes no u8/u16
/// readers, and `split.count`/`split.no` are written as u16 by the HF
/// converter.
struct Header(NonNull<llama_cpp_sys_2::gguf_context>);

impl Header {
    fn open(path: &Path) -> Option<Self> {
        let c_path = CString::new(path.to_str()?).ok()?;
        let params = llama_cpp_sys_2::gguf_init_params {
            no_alloc: true,
            ctx: std::ptr::null_mut(),
        };
        // SAFETY: `c_path` is a valid NUL-terminated string for the duration
        // of the call; `params.ctx` null means ggml allocates nothing for
        // tensor data.
        let ptr = unsafe { llama_cpp_sys_2::gguf_init_from_file(c_path.as_ptr(), params) };
        NonNull::new(ptr).map(Self)
    }

    fn find_key(&self, key: &str) -> i64 {
        let Ok(c_key) = CString::new(key) else {
            return -1;
        };
        // SAFETY: live context, valid C string.
        unsafe { llama_cpp_sys_2::gguf_find_key(self.0.as_ptr(), c_key.as_ptr()) }
    }

    fn kv_type(&self, idx: i64) -> llama_cpp_sys_2::gguf_type {
        // SAFETY: `idx` came from `find_key` on this context and is >= 0.
        unsafe { llama_cpp_sys_2::gguf_get_kv_type(self.0.as_ptr(), idx) }
    }

    fn val_str(&self, idx: i64) -> Option<&str> {
        // SAFETY: caller checked the KV is a string; ggml returns a pointer
        // into the context, which outlives the returned borrow.
        let ptr = unsafe { llama_cpp_sys_2::gguf_get_val_str(self.0.as_ptr(), idx) };
        if ptr.is_null() {
            return None;
        }
        unsafe { CStr::from_ptr(ptr).to_str().ok() }
    }
}

impl Drop for Header {
    fn drop(&mut self) {
        // SAFETY: pointer came from `gguf_init_from_file` and is freed once.
        unsafe { llama_cpp_sys_2::gguf_free(self.0.as_ptr()) }
    }
}

fn chat_template(ctx: &Header) -> Option<String> {
    str_value(ctx, "tokenizer.chat_template")
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
}

fn str_value<'a>(ctx: &'a Header, key: &str) -> Option<&'a str> {
    let idx = ctx.find_key(key);
    if idx < 0 || ctx.kv_type(idx) != llama_cpp_sys_2::GGUF_TYPE_STRING {
        return None;
    }
    ctx.val_str(idx)
}

/// Integer KVs are written with whatever width the converter chose (u32 for
/// context length, u16 for split metadata, …); accept the common ones.
fn u32_value(ctx: &Header, key: &str) -> Option<u32> {
    let idx = ctx.find_key(key);
    if idx < 0 {
        return None;
    }
    let p = ctx.0.as_ptr();
    // SAFETY: each accessor is only called after `kv_type` reported that
    // exact type for `idx`, which is what ggml requires.
    unsafe {
        match ctx.kv_type(idx) {
            llama_cpp_sys_2::GGUF_TYPE_UINT32 => Some(llama_cpp_sys_2::gguf_get_val_u32(p, idx)),
            llama_cpp_sys_2::GGUF_TYPE_INT32 => {
                u32::try_from(llama_cpp_sys_2::gguf_get_val_i32(p, idx)).ok()
            }
            llama_cpp_sys_2::GGUF_TYPE_UINT64 => {
                u32::try_from(llama_cpp_sys_2::gguf_get_val_u64(p, idx)).ok()
            }
            llama_cpp_sys_2::GGUF_TYPE_UINT16 => {
                Some(u32::from(llama_cpp_sys_2::gguf_get_val_u16(p, idx)))
            }
            llama_cpp_sys_2::GGUF_TYPE_UINT8 => {
                Some(u32::from(llama_cpp_sys_2::gguf_get_val_u8(p, idx)))
            }
            _ => None,
        }
    }
}

/// `general.file_type` → the quantization label users know from file names.
/// Mirrors `llama_ftype` in llama.h; unknown values yield `None`.
fn quantization_name(ftype: u32) -> Option<String> {
    let name = match ftype {
        0 => "F32",
        1 => "F16",
        2 => "Q4_0",
        3 => "Q4_1",
        7 => "Q8_0",
        8 => "Q5_0",
        9 => "Q5_1",
        10 => "Q2_K",
        11 => "Q3_K_S",
        12 => "Q3_K_M",
        13 => "Q3_K_L",
        14 => "Q4_K_S",
        15 => "Q4_K_M",
        16 => "Q5_K_S",
        17 => "Q5_K_M",
        18 => "Q6_K",
        19 => "IQ2_XXS",
        20 => "IQ2_XS",
        21 => "Q2_K_S",
        22 => "IQ3_XS",
        23 => "IQ3_XXS",
        24 => "IQ1_S",
        25 => "IQ4_NL",
        26 => "IQ3_S",
        27 => "IQ3_M",
        28 => "IQ2_S",
        29 => "IQ2_M",
        30 => "IQ4_XS",
        31 => "IQ1_M",
        32 => "BF16",
        36 => "TQ1_0",
        37 => "TQ2_0",
        38 => "MXFP4",
        _ => return None,
    };
    Some(name.to_owned())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Minimal GGUF v3 writer: header + string/u32 KVs, zero tensors. Enough
    /// to exercise the reader without checking a real model into the repo.
    pub(crate) enum Kv<'a> {
        Str(&'a str, &'a str),
        U32(&'a str, u32),
        U16(&'a str, u16),
    }

    pub(crate) fn write_gguf(path: &Path, kvs: &[Kv<'_>]) {
        let mut out = Vec::new();
        out.extend_from_slice(b"GGUF");
        out.extend_from_slice(&3u32.to_le_bytes());
        out.extend_from_slice(&0u64.to_le_bytes()); // n_tensors
        out.extend_from_slice(&(kvs.len() as u64).to_le_bytes());
        let put_str = |out: &mut Vec<u8>, s: &str| {
            out.extend_from_slice(&(s.len() as u64).to_le_bytes());
            out.extend_from_slice(s.as_bytes());
        };
        for kv in kvs {
            match kv {
                Kv::Str(k, v) => {
                    put_str(&mut out, k);
                    out.extend_from_slice(&8u32.to_le_bytes()); // GGUF_TYPE_STRING
                    put_str(&mut out, v);
                }
                Kv::U32(k, v) => {
                    put_str(&mut out, k);
                    out.extend_from_slice(&4u32.to_le_bytes()); // GGUF_TYPE_UINT32
                    out.extend_from_slice(&v.to_le_bytes());
                }
                Kv::U16(k, v) => {
                    put_str(&mut out, k);
                    out.extend_from_slice(&2u32.to_le_bytes()); // GGUF_TYPE_UINT16
                    out.extend_from_slice(&v.to_le_bytes());
                }
            }
        }
        std::fs::write(path, out).unwrap();
    }

    const CHATML: &str = "{% for message in messages %}{{'<|im_start|>' + message['role'] + '\n' + message['content'] + '<|im_end|>' + '\n'}}{% endfor %}{% if add_generation_prompt %}{{ '<|im_start|>assistant\n' }}{% endif %}";

    #[test]
    fn reads_every_field_from_a_well_formed_header() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.gguf");
        write_gguf(
            &path,
            &[
                Kv::Str("general.architecture", "qwen2"),
                Kv::Str("general.name", "Qwen2.5 7B Instruct"),
                Kv::Str("general.size_label", "7B"),
                Kv::U32("general.file_type", 15),
                Kv::U32("qwen2.context_length", 32768),
                Kv::Str("tokenizer.chat_template", CHATML),
            ],
        );
        let info = inspect(&path).unwrap();
        assert_eq!(info.architecture, "qwen2");
        assert_eq!(info.name.as_deref(), Some("Qwen2.5 7B Instruct"));
        assert_eq!(info.size_label.as_deref(), Some("7B"));
        assert_eq!(info.quantization.as_deref(), Some("Q4_K_M"));
        assert_eq!(info.context_length, Some(32768));
        assert!(info.embedded_template_supported);
        assert_eq!(info.size_bytes, std::fs::metadata(&path).unwrap().len());
        assert_eq!(
            embedded_chat_template(&path).unwrap().as_deref(),
            Some(CHATML)
        );
    }

    #[test]
    fn missing_template_is_reported_as_unsupported() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.gguf");
        write_gguf(&path, &[Kv::Str("general.architecture", "llama")]);
        let info = inspect(&path).unwrap();
        assert!(!info.embedded_template_supported);
        assert_eq!(info.name, None);
        assert_eq!(info.context_length, None);
        assert_eq!(embedded_chat_template(&path).unwrap(), None);
    }

    #[test]
    fn unrecognised_template_is_reported_as_unsupported() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.gguf");
        write_gguf(
            &path,
            &[
                Kv::Str("general.architecture", "llama"),
                Kv::Str(
                    "tokenizer.chat_template",
                    "{{ something llama.cpp never saw }}",
                ),
            ],
        );
        assert!(!inspect(&path).unwrap().embedded_template_supported);
    }

    #[test]
    fn split_shards_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m-00001-of-00003.gguf");
        write_gguf(
            &path,
            &[
                Kv::Str("general.architecture", "llama"),
                // The HF converter writes split metadata as u16.
                Kv::U16("split.no", 0),
                Kv::U16("split.count", 3),
            ],
        );
        match inspect(&path).unwrap_err() {
            GgufError::Split { no, count } => {
                assert_eq!((no, count), (1, 3));
            }
            other => panic!("expected Split, got {other:?}"),
        }
    }

    #[test]
    fn garbage_file_is_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not.gguf");
        std::fs::write(&path, b"definitely not a gguf").unwrap();
        assert!(matches!(inspect(&path).unwrap_err(), GgufError::Invalid));
    }

    #[test]
    fn missing_file_is_not_found() {
        let err = inspect(Path::new("/nonexistent/x.gguf")).unwrap_err();
        assert!(matches!(err, GgufError::NotFound(_)));
    }

    #[test]
    fn quantization_table_covers_common_ftypes() {
        assert_eq!(quantization_name(15).as_deref(), Some("Q4_K_M"));
        assert_eq!(quantization_name(7).as_deref(), Some("Q8_0"));
        assert_eq!(quantization_name(999), None);
    }
}
