//! MCP adapter module
//!
//! Adapts MCP resources, prompts, and tools to BitFun's agentic system.

pub mod context;
pub mod prompt;
pub mod resource;
pub mod tool;

pub use context::{ContextEnhancer, MCPContextProvider};
pub use prompt::PromptAdapter;
pub use resource::ResourceAdapter;
pub use tool::MCPToolAdapter;
