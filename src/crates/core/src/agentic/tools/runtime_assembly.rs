//! Core-owned product tool runtime assembly.
//!
//! This module is the single core-side owner for assembling the product tool
//! registry while concrete tools, `ToolUseContext`, runtime manifest assembly,
//! and snapshot decoration remain core-owned.

use crate::agentic::tools::registry::{ToolDecoratorRef, ToolRef, ToolRegistry};
use crate::agentic::tools::static_providers::builtin_static_tool_providers;
#[cfg(test)]
use bitfun_agent_tools::StaticToolProvider;
use bitfun_agent_tools::ToolDecorator;
use std::sync::Arc;

#[derive(Clone)]
pub(in crate::agentic::tools) struct ProductToolRuntimeAssembly {
    tool_decorator: ToolDecoratorRef,
}

impl Default for ProductToolRuntimeAssembly {
    fn default() -> Self {
        Self::new()
    }
}

impl ProductToolRuntimeAssembly {
    pub(in crate::agentic::tools) fn new() -> Self {
        Self::with_tool_decorator(Arc::new(SnapshotToolDecorator))
    }

    pub(in crate::agentic::tools) fn with_tool_decorator(tool_decorator: ToolDecoratorRef) -> Self {
        Self { tool_decorator }
    }

    #[cfg(test)]
    pub(in crate::agentic::tools) fn provider_group_ids(&self) -> Vec<&'static str> {
        builtin_static_tool_providers()
            .iter()
            .map(|provider| provider.provider_id())
            .collect()
    }

    pub(in crate::agentic::tools) fn create_registry(&self) -> ToolRegistry {
        let mut registry = ToolRegistry::empty_with_tool_decorator(self.tool_decorator.clone());
        for provider in builtin_static_tool_providers() {
            registry.install_static_provider(&provider);
        }
        registry
    }
}

#[derive(Debug, Clone)]
struct SnapshotToolDecorator;

impl ToolDecorator<ToolRef> for SnapshotToolDecorator {
    fn decorate(&self, tool: ToolRef) -> ToolRef {
        crate::service::snapshot::wrap_tool_for_snapshot_tracking(tool)
    }
}
