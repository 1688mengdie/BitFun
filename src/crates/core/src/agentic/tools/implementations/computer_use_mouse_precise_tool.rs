//! Absolute pointer positioning for Claw Computer use.

use crate::agentic::tools::computer_use_capability::computer_use_desktop_available;
use crate::agentic::tools::implementations::computer_use_tool::computer_use_execute_mouse_precise;
use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::service::config::global::GlobalConfigManager;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct ComputerUseMousePreciseTool;

impl ComputerUseMousePreciseTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ComputerUseMousePreciseTool {
    fn name(&self) -> &str {
        "ComputerUseMousePrecise"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            "Move the mouse pointer to **absolute** coordinates. Use **`coordinate_mode`** (`image` = last screenshot JPEG — **preferred for precision**; `normalized` = 0..1000 — **coarse**, avoid for fine alignment) or **`use_screen_coordinates`** for global display units. Same semantics as the former `ComputerUse` `mouse_move` absolute path. For **small** cardinal nudges, prefer **`ComputerUseMouseStep`** instead of tiny absolute x/y.".to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "x": {
                    "type": "integer",
                    "description": "Target x: in **image** mode, pixel on the latest screenshot JPEG; in **normalized**, 0..=1000 on the captured display; with **use_screen_coordinates**, global display units (host native, e.g. macOS points)."
                },
                "y": { "type": "integer", "description": "Target y; same coordinate space as x." },
                "coordinate_mode": {
                    "type": "string",
                    "enum": ["image", "normalized"],
                    "description": "When use_screen_coordinates is false. \"image\" = pixels on the latest screenshot JPEG (use for precise moves). \"normalized\" = 0..=1000 (coarse grid only)."
                },
                "use_screen_coordinates": {
                    "type": "boolean",
                    "description": "If true, x/y are global display coordinates in the host's native units (on macOS: **points**)."
                }
            },
            "required": ["x", "y"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        false
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        true
    }

    async fn is_enabled(&self) -> bool {
        if !computer_use_desktop_available() {
            return false;
        }
        let Ok(service) = GlobalConfigManager::get_service().await else {
            return false;
        };
        let ai: crate::service::config::types::AIConfig =
            service.get_config(Some("ai")).await.unwrap_or_default();
        ai.computer_use_enabled
    }

    async fn call_impl(&self, input: &Value, context: &ToolUseContext) -> BitFunResult<Vec<ToolResult>> {
        if context.agent_type.as_deref() != Some("Claw") {
            return Err(BitFunError::tool(
                "ComputerUseMousePrecise is only available in Claw assistant mode.".to_string(),
            ));
        }
        if context.is_remote() {
            return Err(BitFunError::tool(
                "ComputerUseMousePrecise cannot run while the session workspace is remote (SSH)."
                    .to_string(),
            ));
        }
        let host = context.computer_use_host.as_ref().ok_or_else(|| {
            BitFunError::tool("Computer use is only available in the BitFun desktop app.".to_string())
        })?;

        computer_use_execute_mouse_precise(host.as_ref(), input).await
    }
}
