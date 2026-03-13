//! Mode system for BitFun
//!
//! Provides flexible mode selection with different system prompts and tool sets

mod custom_subagents;
mod prompt_builder;
mod registry;
// Modes
mod agentic_mode;
mod claw_mode;
mod cowork_mode;
mod debug_mode;
mod plan_mode;
// Built-in subagents
mod explore_agent;
mod file_finder_agent;
// Hidden agents
mod code_review_agent;
mod generate_doc_agent;

use crate::util::errors::{BitFunError, BitFunResult};
pub use agentic_mode::AgenticMode;
use async_trait::async_trait;
pub use claw_mode::ClawMode;
pub use code_review_agent::CodeReviewAgent;
pub use cowork_mode::CoworkMode;
pub use custom_subagents::{CustomSubagent, CustomSubagentKind};
pub use debug_mode::DebugMode;
pub use explore_agent::ExploreAgent;
pub use file_finder_agent::FileFinderAgent;
pub use generate_doc_agent::GenerateDocAgent;
pub use plan_mode::PlanMode;
pub use prompt_builder::PromptBuilder;
pub use registry::{
    get_agent_registry, AgentCategory, AgentInfo, AgentRegistry, CustomSubagentConfig,
    SubAgentSource,
};
use std::any::Any;

// Include embedded prompts generated at compile time
include!(concat!(env!("OUT_DIR"), "/embedded_agents_prompt.rs"));

/// Agent trait defining the interface for all agents
#[async_trait]
pub trait Agent: Send + Sync + 'static {
    /// downcast to specific type
    fn as_any(&self) -> &dyn Any;

    /// Unique identifier for the agent
    fn id(&self) -> &str;

    /// Human-readable name
    fn name(&self) -> &str;

    /// Description of what the agent does
    fn description(&self) -> &str;

    /// Prompt template name for the agent
    fn prompt_template_name(&self) -> &str;

    /// Prompt template name override for a specific model.
    fn prompt_template_name_for_model(&self, _model_name: Option<&str>) -> Option<&str> {
        None
    }

    fn system_reminder_template_name(&self) -> Option<&str> {
        None // by default, no system reminder
    }

    /// Build the system prompt for this agent
    async fn build_prompt(&self, workspace_path: &str) -> BitFunResult<String> {
        let prompt_components = PromptBuilder::new(workspace_path);

        let system_prompt_template =
            get_embedded_prompt(self.prompt_template_name()).ok_or_else(|| {
                BitFunError::Agent(format!(
                    "{} not found in embedded files",
                    self.prompt_template_name()
                ))
            })?;

        let prompt = prompt_components
            .build_prompt_from_template(system_prompt_template)
            .await?;

        Ok(prompt)
    }

    /// Get the system prompt for this agent
    async fn get_system_prompt(&self, workspace_path: Option<&str>) -> BitFunResult<String> {
        if let Some(workspace_path) = workspace_path {
            self.build_prompt(workspace_path).await
        } else {
            Err(BitFunError::Agent("Workspace path is required".to_string()))
        }
    }

    /// Get the system prompt for this agent with optional model-aware template selection.
    async fn get_system_prompt_for_model(
        &self,
        workspace_path: Option<&str>,
        model_name: Option<&str>,
    ) -> BitFunResult<String> {
        let Some(workspace_path) = workspace_path else {
            return Err(BitFunError::Agent("Workspace path is required".to_string()));
        };

        let Some(template_name) = self.prompt_template_name_for_model(model_name) else {
            return self.build_prompt(workspace_path).await;
        };

        let prompt_components = PromptBuilder::new(workspace_path);
        let system_prompt_template = get_embedded_prompt(template_name).ok_or_else(|| {
            BitFunError::Agent(format!("{} not found in embedded files", template_name))
        })?;

        prompt_components
            .build_prompt_from_template(system_prompt_template)
            .await
    }

    /// Get the system reminder for this agent, only used for modes
    /// system_reminder will be appended to the user_query
    /// This is not necessary for all modes
    /// index is not used for now (Cursor first time enter plan mode and keep plan mode will use different reminder)
    async fn get_system_reminder(&self, _index: usize) -> BitFunResult<String> {
        if let Some(system_reminder_template_name) = self.system_reminder_template_name() {
            let system_reminder =
                get_embedded_prompt(system_reminder_template_name).ok_or_else(|| {
                    BitFunError::Agent(format!(
                        "{} not found in embedded files",
                        system_reminder_template_name
                    ))
                })?;
            Ok(system_reminder.to_string())
        } else {
            Ok("".to_string())
        }
    }

    /// Get the list of default tools for this agent
    fn default_tools(&self) -> Vec<String>;

    /// Whether this agent is read-only (prevents file modifications)
    fn is_readonly(&self) -> bool {
        false
    }
}
