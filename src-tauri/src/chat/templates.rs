//! Chat-template rendering for catalog model families.
//!
//! The catalog stores a coarse model-family template, not a tokenizer-specific
//! Jinja template. Keep these renderers conservative: when a family has
//! incompatible sub-templates, expose explicit options or return a documented
//! placeholder error instead of silently mapping it to a nearby format.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

use crate::models::catalog::ChatTemplate;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

impl ChatRole {
    fn as_chatml(self) -> &'static str {
        match self {
            ChatRole::System => "system",
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
            ChatRole::Tool => "tool",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

impl ChatMessage {
    pub fn new(role: ChatRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOptions {
    pub add_generation_prompt: bool,
    pub qwen3_thinking: bool,
    pub deepseek_thinking: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            add_generation_prompt: true,
            qwen3_thinking: false,
            deepseek_thinking: false,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TemplateError {
    #[error("{template:?} does not support {role:?} messages")]
    UnsupportedRole {
        template: ChatTemplate,
        role: ChatRole,
    },
    #[error("{template:?} template is not implemented yet: {reason}")]
    Unimplemented {
        template: ChatTemplate,
        reason: &'static str,
    },
}

pub fn render_chat_template(
    template: ChatTemplate,
    messages: &[ChatMessage],
) -> Result<String, TemplateError> {
    render_chat_template_with_options(template, messages, RenderOptions::default())
}

pub fn render_chat_template_with_options(
    template: ChatTemplate,
    messages: &[ChatMessage],
    options: RenderOptions,
) -> Result<String, TemplateError> {
    match template {
        ChatTemplate::ChatML => render_chatml(messages, options.add_generation_prompt),
        ChatTemplate::Qwen => render_chatml(messages, options.add_generation_prompt),
        ChatTemplate::Qwen3 => render_qwen3(messages, options),
        ChatTemplate::Llama3 => render_llama3(messages, options.add_generation_prompt),
        ChatTemplate::Llama2 => render_llama2(messages, options.add_generation_prompt),
        ChatTemplate::Mistral => render_mistral(messages, options.add_generation_prompt),
        ChatTemplate::Gemma => render_gemma(messages, options.add_generation_prompt),
        ChatTemplate::Gemma4 => Err(TemplateError::Unimplemented {
            template,
            reason: "TODO: confirm the Gemma4 tokenizer chat_template before emitting prompts",
        }),
        ChatTemplate::Phi3 => render_phi3(messages, options.add_generation_prompt),
        ChatTemplate::DeepSeek => render_deepseek(messages, options),
        ChatTemplate::CommandR => render_command_r(messages, options.add_generation_prompt),
        ChatTemplate::GLM4 => render_glm4(messages, options.add_generation_prompt),
    }
}

fn render_chatml(
    messages: &[ChatMessage],
    add_generation_prompt: bool,
) -> Result<String, TemplateError> {
    let mut out = String::new();
    for message in messages {
        out.push_str("<|im_start|>");
        out.push_str(message.role.as_chatml());
        out.push('\n');
        out.push_str(&message.content);
        out.push_str("<|im_end|>\n");
    }
    if add_generation_prompt {
        out.push_str("<|im_start|>assistant\n");
    }
    Ok(out)
}

fn render_qwen3(messages: &[ChatMessage], options: RenderOptions) -> Result<String, TemplateError> {
    let mut out = render_chatml(messages, false)?;
    if options.add_generation_prompt {
        out.push_str("<|im_start|>assistant\n");
        if !options.qwen3_thinking {
            // Qwen3's non-thinking mode is distinct from generic Qwen/ChatML:
            // the tokenizer template suppresses reasoning by opening and
            // immediately closing an empty think block at generation start.
            out.push_str("<think>\n\n</think>\n\n");
        }
    }
    Ok(out)
}

fn render_llama3(
    messages: &[ChatMessage],
    add_generation_prompt: bool,
) -> Result<String, TemplateError> {
    let mut out = String::from("<|begin_of_text|>");
    for message in messages {
        out.push_str("<|start_header_id|>");
        out.push_str(message.role.as_chatml());
        out.push_str("<|end_header_id|>\n\n");
        out.push_str(&message.content);
        out.push_str("<|eot_id|>");
    }
    if add_generation_prompt {
        out.push_str("<|start_header_id|>assistant<|end_header_id|>\n\n");
    }
    Ok(out)
}

fn render_llama2(
    messages: &[ChatMessage],
    add_generation_prompt: bool,
) -> Result<String, TemplateError> {
    render_inst_template(ChatTemplate::Llama2, messages, add_generation_prompt, true)
}

fn render_mistral(
    messages: &[ChatMessage],
    add_generation_prompt: bool,
) -> Result<String, TemplateError> {
    render_inst_template(
        ChatTemplate::Mistral,
        messages,
        add_generation_prompt,
        false,
    )
}

fn render_inst_template(
    template: ChatTemplate,
    messages: &[ChatMessage],
    add_generation_prompt: bool,
    llama2_system_block: bool,
) -> Result<String, TemplateError> {
    let mut system = Vec::new();
    let mut turns = Vec::new();
    for message in messages {
        match message.role {
            ChatRole::System => system.push(message.content.as_str()),
            ChatRole::User | ChatRole::Assistant => turns.push(message),
            ChatRole::Tool => {
                return Err(TemplateError::UnsupportedRole {
                    template,
                    role: message.role,
                })
            }
        }
    }

    let system = system.join("\n\n");
    let mut out = String::new();
    let mut first_user = true;
    let mut expecting_assistant = false;

    for message in turns {
        match message.role {
            ChatRole::User => {
                if template == ChatTemplate::Llama2 || out.is_empty() {
                    out.push_str("<s>");
                }
                out.push_str("[INST] ");
                if first_user && !system.is_empty() {
                    if llama2_system_block {
                        out.push_str("<<SYS>>\n");
                        out.push_str(&system);
                        out.push_str("\n<</SYS>>\n\n");
                    } else {
                        out.push_str(&system);
                        out.push_str("\n\n");
                    }
                }
                out.push_str(&message.content);
                out.push_str(" [/INST]");
                first_user = false;
                expecting_assistant = true;
            }
            ChatRole::Assistant => {
                if expecting_assistant {
                    out.push(' ');
                    out.push_str(&message.content);
                    out.push_str("</s>");
                    expecting_assistant = false;
                }
            }
            ChatRole::System | ChatRole::Tool => unreachable!(),
        }
    }

    if !add_generation_prompt && expecting_assistant {
        out.push(' ');
    }
    Ok(out)
}

fn render_gemma(
    messages: &[ChatMessage],
    add_generation_prompt: bool,
) -> Result<String, TemplateError> {
    let mut system = Vec::new();
    let mut out = String::new();
    let mut pending_system = true;

    for message in messages {
        match message.role {
            ChatRole::System if pending_system => system.push(message.content.as_str()),
            ChatRole::System => {
                return Err(TemplateError::UnsupportedRole {
                    template: ChatTemplate::Gemma,
                    role: ChatRole::System,
                })
            }
            ChatRole::User => {
                out.push_str("<start_of_turn>user\n");
                if !system.is_empty() {
                    out.push_str(&system.join("\n\n"));
                    out.push_str("\n\n");
                    system.clear();
                }
                out.push_str(&message.content);
                out.push_str("<end_of_turn>\n");
                pending_system = false;
            }
            ChatRole::Assistant => {
                out.push_str("<start_of_turn>model\n");
                out.push_str(&message.content);
                out.push_str("<end_of_turn>\n");
                pending_system = false;
            }
            ChatRole::Tool => {
                return Err(TemplateError::UnsupportedRole {
                    template: ChatTemplate::Gemma,
                    role: ChatRole::Tool,
                })
            }
        }
    }

    if add_generation_prompt {
        out.push_str("<start_of_turn>model\n");
    }
    Ok(out)
}

fn render_phi3(
    messages: &[ChatMessage],
    add_generation_prompt: bool,
) -> Result<String, TemplateError> {
    let mut out = String::new();
    for message in messages {
        let role = match message.role {
            ChatRole::System => "system",
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
            ChatRole::Tool => {
                return Err(TemplateError::UnsupportedRole {
                    template: ChatTemplate::Phi3,
                    role: ChatRole::Tool,
                })
            }
        };
        out.push_str("<|");
        out.push_str(role);
        out.push_str("|>\n");
        out.push_str(&message.content);
        out.push_str("<|end|>\n");
    }
    if add_generation_prompt {
        out.push_str("<|assistant|>\n");
    }
    Ok(out)
}

fn render_deepseek(
    messages: &[ChatMessage],
    options: RenderOptions,
) -> Result<String, TemplateError> {
    let mut out = String::from("<｜begin▁of▁sentence｜>");
    for message in messages {
        match message.role {
            ChatRole::System => out.push_str(&message.content),
            ChatRole::User => {
                out.push_str("<｜User｜>");
                out.push_str(&message.content);
            }
            ChatRole::Assistant => {
                out.push_str("<｜Assistant｜></think>");
                out.push_str(&message.content);
                out.push_str("<｜end▁of▁sentence｜>");
            }
            ChatRole::Tool => {
                return Err(TemplateError::UnsupportedRole {
                    template: ChatTemplate::DeepSeek,
                    role: ChatRole::Tool,
                })
            }
        }
    }
    if options.add_generation_prompt {
        out.push_str("<｜Assistant｜>");
        if options.deepseek_thinking {
            out.push_str("<think>");
        } else {
            out.push_str("</think>");
        }
    }
    Ok(out)
}

fn render_command_r(
    messages: &[ChatMessage],
    add_generation_prompt: bool,
) -> Result<String, TemplateError> {
    let mut out = String::new();
    for message in messages {
        let role = match message.role {
            ChatRole::System => "<|SYSTEM_TOKEN|>",
            ChatRole::User => "<|USER_TOKEN|>",
            ChatRole::Assistant => "<|CHATBOT_TOKEN|>",
            ChatRole::Tool => {
                return Err(TemplateError::UnsupportedRole {
                    template: ChatTemplate::CommandR,
                    role: ChatRole::Tool,
                })
            }
        };
        out.push_str("<|START_OF_TURN_TOKEN|>");
        out.push_str(role);
        out.push_str(&message.content);
        out.push_str("<|END_OF_TURN_TOKEN|>");
    }
    if add_generation_prompt {
        out.push_str("<|START_OF_TURN_TOKEN|><|CHATBOT_TOKEN|>");
    }
    Ok(out)
}

fn render_glm4(
    messages: &[ChatMessage],
    add_generation_prompt: bool,
) -> Result<String, TemplateError> {
    let mut out = String::from("[gMASK]<sop>");
    for message in messages {
        let role = match message.role {
            ChatRole::System => "system",
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
            ChatRole::Tool => {
                return Err(TemplateError::UnsupportedRole {
                    template: ChatTemplate::GLM4,
                    role: ChatRole::Tool,
                })
            }
        };
        out.push_str("<|");
        out.push_str(role);
        out.push_str("|>\n");
        out.push_str(&message.content);
    }
    if add_generation_prompt {
        out.push_str("<|assistant|>\n");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_messages() -> Vec<ChatMessage> {
        vec![
            ChatMessage::new(ChatRole::System, "You are concise."),
            ChatMessage::new(ChatRole::User, "Hello"),
            ChatMessage::new(ChatRole::Assistant, "Hi"),
            ChatMessage::new(ChatRole::User, "Bye"),
        ]
    }

    #[test]
    fn renders_chatml() {
        assert_eq!(
            render_chat_template(ChatTemplate::ChatML, &sample_messages()).unwrap(),
            "<|im_start|>system\nYou are concise.<|im_end|>\n<|im_start|>user\nHello<|im_end|>\n<|im_start|>assistant\nHi<|im_end|>\n<|im_start|>user\nBye<|im_end|>\n<|im_start|>assistant\n"
        );
    }

    #[test]
    fn renders_qwen_generic_as_chatml() {
        assert_eq!(
            render_chat_template(ChatTemplate::Qwen, &sample_messages()).unwrap(),
            render_chat_template(ChatTemplate::ChatML, &sample_messages()).unwrap()
        );
    }

    #[test]
    fn renders_qwen3_non_thinking_distinct_from_qwen() {
        let prompt = render_chat_template(ChatTemplate::Qwen3, &sample_messages()).unwrap();
        assert_eq!(
            prompt,
            "<|im_start|>system\nYou are concise.<|im_end|>\n<|im_start|>user\nHello<|im_end|>\n<|im_start|>assistant\nHi<|im_end|>\n<|im_start|>user\nBye<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
        );
    }

    #[test]
    fn renders_qwen3_thinking() {
        let prompt = render_chat_template_with_options(
            ChatTemplate::Qwen3,
            &sample_messages(),
            RenderOptions {
                qwen3_thinking: true,
                ..RenderOptions::default()
            },
        )
        .unwrap();
        assert_eq!(
            prompt,
            "<|im_start|>system\nYou are concise.<|im_end|>\n<|im_start|>user\nHello<|im_end|>\n<|im_start|>assistant\nHi<|im_end|>\n<|im_start|>user\nBye<|im_end|>\n<|im_start|>assistant\n"
        );
    }

    #[test]
    fn renders_llama3() {
        assert_eq!(
            render_chat_template(ChatTemplate::Llama3, &sample_messages()).unwrap(),
            "<|begin_of_text|><|start_header_id|>system<|end_header_id|>\n\nYou are concise.<|eot_id|><|start_header_id|>user<|end_header_id|>\n\nHello<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\nHi<|eot_id|><|start_header_id|>user<|end_header_id|>\n\nBye<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n"
        );
    }

    #[test]
    fn renders_llama2() {
        assert_eq!(
            render_chat_template(ChatTemplate::Llama2, &sample_messages()).unwrap(),
            "<s>[INST] <<SYS>>\nYou are concise.\n<</SYS>>\n\nHello [/INST] Hi</s><s>[INST] Bye [/INST]"
        );
    }

    #[test]
    fn renders_mistral() {
        assert_eq!(
            render_chat_template(ChatTemplate::Mistral, &sample_messages()).unwrap(),
            "<s>[INST] You are concise.\n\nHello [/INST] Hi</s>[INST] Bye [/INST]"
        );
    }

    #[test]
    fn renders_gemma() {
        assert_eq!(
            render_chat_template(ChatTemplate::Gemma, &sample_messages()).unwrap(),
            "<start_of_turn>user\nYou are concise.\n\nHello<end_of_turn>\n<start_of_turn>model\nHi<end_of_turn>\n<start_of_turn>user\nBye<end_of_turn>\n<start_of_turn>model\n"
        );
    }

    #[test]
    fn gemma4_placeholder_is_explicit() {
        assert_eq!(
            render_chat_template(ChatTemplate::Gemma4, &sample_messages()).unwrap_err(),
            TemplateError::Unimplemented {
                template: ChatTemplate::Gemma4,
                reason: "TODO: confirm the Gemma4 tokenizer chat_template before emitting prompts",
            }
        );
    }

    #[test]
    fn renders_phi3() {
        assert_eq!(
            render_chat_template(ChatTemplate::Phi3, &sample_messages()).unwrap(),
            "<|system|>\nYou are concise.<|end|>\n<|user|>\nHello<|end|>\n<|assistant|>\nHi<|end|>\n<|user|>\nBye<|end|>\n<|assistant|>\n"
        );
    }

    #[test]
    fn renders_deepseek_non_thinking() {
        assert_eq!(
            render_chat_template(ChatTemplate::DeepSeek, &sample_messages()).unwrap(),
            "<｜begin▁of▁sentence｜>You are concise.<｜User｜>Hello<｜Assistant｜></think>Hi<｜end▁of▁sentence｜><｜User｜>Bye<｜Assistant｜></think>"
        );
    }

    #[test]
    fn renders_deepseek_thinking() {
        let prompt = render_chat_template_with_options(
            ChatTemplate::DeepSeek,
            &sample_messages(),
            RenderOptions {
                deepseek_thinking: true,
                ..RenderOptions::default()
            },
        )
        .unwrap();
        assert_eq!(
            prompt,
            "<｜begin▁of▁sentence｜>You are concise.<｜User｜>Hello<｜Assistant｜></think>Hi<｜end▁of▁sentence｜><｜User｜>Bye<｜Assistant｜><think>"
        );
    }

    #[test]
    fn renders_command_r() {
        assert_eq!(
            render_chat_template(ChatTemplate::CommandR, &sample_messages()).unwrap(),
            "<|START_OF_TURN_TOKEN|><|SYSTEM_TOKEN|>You are concise.<|END_OF_TURN_TOKEN|><|START_OF_TURN_TOKEN|><|USER_TOKEN|>Hello<|END_OF_TURN_TOKEN|><|START_OF_TURN_TOKEN|><|CHATBOT_TOKEN|>Hi<|END_OF_TURN_TOKEN|><|START_OF_TURN_TOKEN|><|USER_TOKEN|>Bye<|END_OF_TURN_TOKEN|><|START_OF_TURN_TOKEN|><|CHATBOT_TOKEN|>"
        );
    }

    #[test]
    fn renders_glm4() {
        assert_eq!(
            render_chat_template(ChatTemplate::GLM4, &sample_messages()).unwrap(),
            "[gMASK]<sop><|system|>\nYou are concise.<|user|>\nHello<|assistant|>\nHi<|user|>\nBye<|assistant|>\n"
        );
    }

    #[test]
    fn instruction_templates_reject_tool_role() {
        let messages = [ChatMessage::new(ChatRole::Tool, "tool output")];
        assert_eq!(
            render_chat_template(ChatTemplate::Mistral, &messages).unwrap_err(),
            TemplateError::UnsupportedRole {
                template: ChatTemplate::Mistral,
                role: ChatRole::Tool,
            }
        );
    }
}
