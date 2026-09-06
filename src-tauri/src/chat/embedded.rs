//! Rendering with the chat template embedded in a GGUF.
//!
//! Custom models (imported from disk or downloaded from a user-supplied URL)
//! are not in the catalog, so nobody has hand-picked a `ChatTemplate` family
//! for them. What they do carry is `tokenizer.chat_template`: the Jinja
//! template the model was trained with. llama.cpp's
//! `llama_chat_apply_template` renders those — not with a Jinja engine, but
//! by recognising the template against its list of known families
//! (llama-chat.cpp), which covers the overwhelming majority of instruct GGUFs
//! in circulation. When the template is *not* recognised the call fails and
//! the UI falls back to letting the user pick a family by hand.
//!
//! Two consequences worth keeping in mind:
//!
//! * `supports()` can be answered from the template string alone, so a GGUF
//!   can be classified at import time without loading its weights.
//! * The output never contains a BOS token: llama.cpp deliberately leaves it
//!   to the tokenizer (`add_special = true`), which is how llama-server feeds
//!   these prompts. Hence `BosPolicy::Always` for embedded templates.

use std::ffi::CString;
use std::os::raw::c_char;

use crate::chat::templates::{ChatMessage, ChatRole, TemplateError};

fn role_name(role: ChatRole) -> &'static str {
    match role {
        ChatRole::System => "system",
        ChatRole::User => "user",
        ChatRole::Assistant => "assistant",
        ChatRole::Tool => "tool",
    }
}

/// Render `messages` through `template` with llama.cpp. `add_generation_prompt`
/// appends the tokens that open an assistant turn.
pub fn render(
    template: &str,
    messages: &[ChatMessage],
    add_generation_prompt: bool,
) -> Result<String, TemplateError> {
    if messages.is_empty() {
        return Err(TemplateError::EmptyMessages);
    }

    let tmpl = CString::new(template).map_err(|_| {
        TemplateError::Embedded("chat template contains an interior NUL byte".into())
    })?;

    let owned: Vec<(CString, CString)> = messages
        .iter()
        .map(|m| {
            Ok((
                CString::new(role_name(m.role)).expect("role names have no NUL"),
                CString::new(m.content.as_str()).map_err(|_| {
                    TemplateError::Embedded("message content contains an interior NUL byte".into())
                })?,
            ))
        })
        .collect::<Result<_, TemplateError>>()?;
    let chat: Vec<llama_cpp_sys_2::llama_chat_message> = owned
        .iter()
        .map(|(role, content)| llama_cpp_sys_2::llama_chat_message {
            role: role.as_ptr(),
            content: content.as_ptr(),
        })
        .collect();

    // llama.cpp recommends 2x the message text as the initial buffer; when
    // that's short the call reports the needed size and we retry once.
    let text_len: usize = messages.iter().map(|m| m.content.len() + 16).sum();
    let mut buf: Vec<u8> = vec![0; text_len * 2];

    let mut needed = apply(&tmpl, &chat, add_generation_prompt, &mut buf)?;
    if needed > buf.len() {
        buf.resize(needed, 0);
        needed = apply(&tmpl, &chat, add_generation_prompt, &mut buf)?;
    }
    buf.truncate(needed);

    String::from_utf8(buf)
        .map_err(|e| TemplateError::Embedded(format!("rendered prompt is not UTF-8: {e}")))
}

fn apply(
    tmpl: &CString,
    chat: &[llama_cpp_sys_2::llama_chat_message],
    add_ass: bool,
    buf: &mut [u8],
) -> Result<usize, TemplateError> {
    // SAFETY: `tmpl` and every `role`/`content` pointer in `chat` are owned
    // `CString`s that outlive this call; `buf` is a live, exclusively borrowed
    // slice whose length is passed alongside its pointer.
    let res = unsafe {
        llama_cpp_sys_2::llama_chat_apply_template(
            tmpl.as_ptr(),
            chat.as_ptr(),
            chat.len(),
            add_ass,
            buf.as_mut_ptr().cast::<c_char>(),
            i32::try_from(buf.len()).unwrap_or(i32::MAX),
        )
    };
    usize::try_from(res).map_err(|_| {
        TemplateError::Embedded(
            "llama.cpp does not recognise this model's embedded chat template".into(),
        )
    })
}

/// Whether llama.cpp can render `template`. Decided by rendering a probe
/// conversation — the same call the real render path uses.
pub fn supports(template: &str) -> bool {
    let probe = [
        ChatMessage {
            role: ChatRole::User,
            content: "hi".into(),
        },
        ChatMessage {
            role: ChatRole::Assistant,
            content: "hello".into(),
        },
        ChatMessage {
            role: ChatRole::User,
            content: "?".into(),
        },
    ];
    render(template, &probe, true).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // The ChatML template as shipped in Qwen/OpenHermes GGUFs.
    const CHATML: &str = "{% for message in messages %}{{'<|im_start|>' + message['role'] + '\n' + message['content'] + '<|im_end|>' + '\n'}}{% endfor %}{% if add_generation_prompt %}{{ '<|im_start|>assistant\n' }}{% endif %}";

    fn msgs() -> Vec<ChatMessage> {
        vec![
            ChatMessage {
                role: ChatRole::System,
                content: "Be brief.".into(),
            },
            ChatMessage {
                role: ChatRole::User,
                content: "Hi".into(),
            },
        ]
    }

    #[test]
    fn renders_chatml_and_opens_assistant_turn() {
        let out = render(CHATML, &msgs(), true).unwrap();
        assert_eq!(
            out,
            "<|im_start|>system\nBe brief.<|im_end|>\n<|im_start|>user\nHi<|im_end|>\n<|im_start|>assistant\n"
        );
    }

    #[test]
    fn generation_prompt_is_optional() {
        let out = render(CHATML, &msgs(), false).unwrap();
        assert!(out.ends_with("Hi<|im_end|>\n"));
    }

    #[test]
    fn llama_cpp_names_are_accepted_as_templates() {
        // llama.cpp also accepts the short family names it uses internally.
        assert!(supports("chatml"));
        assert!(supports("llama3"));
    }

    #[test]
    fn unknown_jinja_is_rejected() {
        assert!(!supports("{{ this is not a template llama.cpp knows }}"));
        let err = render("{{ nope }}", &msgs(), true).unwrap_err();
        assert!(matches!(err, TemplateError::Embedded(_)));
    }

    #[test]
    fn empty_conversation_is_an_error() {
        assert_eq!(render(CHATML, &[], true), Err(TemplateError::EmptyMessages));
    }

    #[test]
    fn long_content_grows_the_buffer() {
        let big = "x".repeat(20_000);
        let m = vec![ChatMessage {
            role: ChatRole::User,
            content: big.clone(),
        }];
        let out = render(CHATML, &m, true).unwrap();
        assert!(out.contains(&big));
    }
}
