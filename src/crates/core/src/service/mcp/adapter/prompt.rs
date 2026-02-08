//! MCP prompt adapter
//!
//! Integrates MCP prompts into the agent system prompt.

use crate::service::mcp::protocol::{MCPPrompt, MCPPromptContent, MCPPromptMessage};

/// Prompt adapter.
pub struct PromptAdapter;

impl PromptAdapter {
    /// Converts MCP prompt content into system prompt text.
    pub fn to_system_prompt(content: &MCPPromptContent) -> String {
        let mut prompt_parts = Vec::new();

        for message in &content.messages {
            match message.role.as_str() {
                "system" => {
                    prompt_parts.push(message.content.clone());
                }
                "user" => {
                    prompt_parts.push(format!("User: {}", message.content));
                }
                "assistant" => {
                    prompt_parts.push(format!("Assistant: {}", message.content));
                }
                _ => {
                    prompt_parts.push(format!("{}: {}", message.role, message.content));
                }
            }
        }

        prompt_parts.join("\n\n")
    }

    /// Returns whether a prompt is applicable to the current context.
    pub fn is_applicable(
        prompt: &MCPPrompt,
        context: &std::collections::HashMap<String, String>,
    ) -> bool {
        if let Some(arguments) = &prompt.arguments {
            for arg in arguments {
                if arg.required && !context.contains_key(&arg.name) {
                    return false;
                }
            }
        }
        true
    }

    /// Substitutes arguments in prompt messages.
    pub fn substitute_arguments(
        messages: Vec<MCPPromptMessage>,
        arguments: &std::collections::HashMap<String, String>,
    ) -> Vec<MCPPromptMessage> {
        messages
            .into_iter()
            .map(|mut msg| {
                for (key, value) in arguments {
                    let placeholder = format!("{{{{{}}}}}", key);
                    msg.content = msg.content.replace(&placeholder, value);
                }
                msg
            })
            .collect()
    }
}
