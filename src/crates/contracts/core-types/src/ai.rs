use crate::ToolImageAttachment;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

const MAX_REASONING_SETTING_DEPTH: usize = 32;

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningMode {
    #[default]
    Default,
    Enabled,
    Disabled,
    Adaptive,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum ReasoningCatalogBinding {
    #[default]
    Auto,
    ModelsDev {
        provider: String,
        model: String,
    },
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReasoningPresetSetting {
    /// Sets a provider-neutral mode without inventing an effort or budget.
    Mode {
        value: ReasoningMode,
    },
    Effort {
        value: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode: Option<ReasoningMode>,
    },
    Toggle {
        enabled: bool,
    },
    BudgetTokens {
        value: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode: Option<ReasoningMode>,
    },
    RequestPatch {
        body: Value,
    },
    /// Preserves combinations such as a legacy mode + effort + token budget.
    Sequence {
        settings: Vec<ReasoningPresetSetting>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct ReasoningPreset {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<i32>,
    #[serde(skip_serializing_if = "is_false")]
    pub disabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setting: Option<ReasoningPresetSetting>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct ReasoningConfig {
    pub catalog: ReasoningCatalogBinding,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_preset: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub presets: Vec<ReasoningPreset>,
}

impl ReasoningConfig {
    pub fn preset(&self, preset_id: &str) -> Option<&ReasoningPreset> {
        let preset_id = preset_id.trim();
        self.presets
            .iter()
            .rev()
            .find(|preset| preset.id.trim() == preset_id)
            .filter(|preset| !preset.disabled && preset.setting.is_some())
    }

    pub fn default_preset(&self) -> Option<&ReasoningPreset> {
        self.default_preset
            .as_deref()
            .and_then(|preset_id| self.preset(preset_id))
    }

    /// Validates the provider-neutral canonical reasoning schema.
    ///
    /// Catalog-dependent default resolution is intentionally owned by the
    /// configuration provider because generated presets are not available in
    /// this dependency-light contract crate.
    pub fn validate_schema(&self) -> Result<(), String> {
        if let ReasoningCatalogBinding::ModelsDev { provider, model } = &self.catalog {
            if provider.trim().is_empty() {
                return Err("models.dev catalog provider must not be empty".to_string());
            }
            if model.trim().is_empty() {
                return Err("models.dev catalog model must not be empty".to_string());
            }
        }

        if let Some(default_preset) = self.default_preset.as_deref() {
            if default_preset.trim().is_empty() {
                return Err("default preset ID must not be empty".to_string());
            }
            if default_preset != default_preset.trim() {
                return Err("default preset ID must not contain surrounding whitespace".to_string());
            }
        }

        let mut preset_ids = HashSet::new();
        for (index, preset) in self.presets.iter().enumerate() {
            let preset_id = preset.id.trim();
            if preset_id.is_empty() {
                return Err(format!("preset ID must not be empty at index {index}"));
            }
            if preset.id != preset_id {
                return Err(format!(
                    "preset ID must not contain surrounding whitespace at index {index}"
                ));
            }
            if !preset_ids.insert(preset_id) {
                return Err(format!("duplicate preset ID '{preset_id}'"));
            }

            match preset.setting.as_ref() {
                Some(setting) if !preset.disabled => validate_reasoning_setting(setting, 0)
                    .map_err(|message| format!("invalid preset '{preset_id}': {message}"))?,
                None if !preset.disabled => {
                    return Err(format!(
                        "enabled preset '{preset_id}' must define a setting"
                    ));
                }
                Some(_) | None => {}
            }
        }

        Ok(())
    }
}

fn validate_reasoning_setting(
    setting: &ReasoningPresetSetting,
    depth: usize,
) -> Result<(), String> {
    if depth >= MAX_REASONING_SETTING_DEPTH {
        return Err(format!(
            "setting nesting must not exceed {MAX_REASONING_SETTING_DEPTH} levels"
        ));
    }

    match setting {
        ReasoningPresetSetting::Effort { value, .. } if value.trim().is_empty() => {
            Err("effort value must not be empty".to_string())
        }
        ReasoningPresetSetting::BudgetTokens { value: 0, .. } => {
            Err("budget_tokens value must be greater than 0".to_string())
        }
        ReasoningPresetSetting::RequestPatch { body } if !body.is_object() => {
            Err("request_patch body must be a JSON object".to_string())
        }
        ReasoningPresetSetting::Sequence { settings } if settings.is_empty() => {
            Err("sequence settings must not be empty".to_string())
        }
        ReasoningPresetSetting::Sequence { settings } => {
            for (index, nested) in settings.iter().enumerate() {
                validate_reasoning_setting(nested, depth + 1)
                    .map_err(|message| format!("sequence item {index}: {message}"))?;
            }
            Ok(())
        }
        ReasoningPresetSetting::Mode { .. }
        | ReasoningPresetSetting::Effort { .. }
        | ReasoningPresetSetting::Toggle { .. }
        | ReasoningPresetSetting::BudgetTokens { .. }
        | ReasoningPresetSetting::RequestPatch { .. } => Ok(()),
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningPresetSource {
    ModelsDev,
    AdapterFallback,
    ModelConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReasoningPresetDescriptor {
    pub id: String,
    pub label: String,
    pub order: i32,
    pub setting: ReasoningPresetSetting,
    pub source: ReasoningPresetSource,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningCapabilityStatus {
    Unsupported,
    Known,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReasoningCatalogProjection {
    pub status: ReasoningCapabilityStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_preset: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presets: Vec<ReasoningPresetDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReasoningRuntimeParameters {
    pub mode: ReasoningMode,
    pub effort: Option<String>,
    pub budget_tokens: Option<u32>,
}

impl Default for ReasoningRuntimeParameters {
    fn default() -> Self {
        Self {
            mode: ReasoningMode::Default,
            effort: None,
            budget_tokens: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReasoningPresetApplication {
    pub parameters: Option<ReasoningRuntimeParameters>,
    pub request_patches: Vec<Value>,
}

impl ReasoningPresetSetting {
    pub fn application(&self) -> ReasoningPresetApplication {
        fn apply(
            setting: &ReasoningPresetSetting,
            parameters: &mut Option<ReasoningRuntimeParameters>,
            request_patches: &mut Vec<Value>,
        ) {
            match setting {
                ReasoningPresetSetting::Mode { value } => {
                    parameters.get_or_insert_with(Default::default).mode = *value;
                }
                ReasoningPresetSetting::Effort { value, mode } => {
                    let parameters = parameters.get_or_insert_with(Default::default);
                    parameters.mode = mode.unwrap_or(ReasoningMode::Enabled);
                    parameters.effort = Some(value.clone());
                }
                ReasoningPresetSetting::Toggle { enabled } => {
                    parameters.get_or_insert_with(Default::default).mode = if *enabled {
                        ReasoningMode::Enabled
                    } else {
                        ReasoningMode::Disabled
                    };
                }
                ReasoningPresetSetting::BudgetTokens { value, mode } => {
                    let parameters = parameters.get_or_insert_with(Default::default);
                    parameters.mode = mode.unwrap_or(ReasoningMode::Enabled);
                    parameters.budget_tokens = Some(*value);
                }
                ReasoningPresetSetting::RequestPatch { body } => {
                    request_patches.push(body.clone());
                }
                ReasoningPresetSetting::Sequence { settings } => {
                    for setting in settings {
                        apply(setting, parameters, request_patches);
                    }
                }
            }
        }

        let mut application = ReasoningPresetApplication::default();
        apply(
            self,
            &mut application.parameters,
            &mut application.request_patches,
        );
        application
    }
}

#[cfg(test)]
mod reasoning_tests {
    use serde_json::json;

    use super::{
        ReasoningCatalogBinding, ReasoningConfig, ReasoningMode, ReasoningPreset,
        ReasoningPresetSetting,
    };

    fn config_with(setting: ReasoningPresetSetting) -> ReasoningConfig {
        ReasoningConfig {
            default_preset: Some("custom".to_string()),
            presets: vec![ReasoningPreset {
                id: "custom".to_string(),
                setting: Some(setting),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn sequence_preserves_legacy_mode_effort_and_budget_parameters() {
        let setting = ReasoningPresetSetting::Sequence {
            settings: vec![
                ReasoningPresetSetting::Effort {
                    value: "high".to_string(),
                    mode: Some(ReasoningMode::Adaptive),
                },
                ReasoningPresetSetting::BudgetTokens {
                    value: 12000,
                    mode: Some(ReasoningMode::Adaptive),
                },
            ],
        };
        let application = setting.application();
        let parameters = application.parameters.expect("semantic parameters");

        assert_eq!(parameters.mode, ReasoningMode::Adaptive);
        assert_eq!(parameters.effort.as_deref(), Some("high"));
        assert_eq!(parameters.budget_tokens, Some(12000));
        assert!(application.request_patches.is_empty());
    }

    #[test]
    fn schema_rejects_duplicate_preset_ids_and_uses_last_definition_defensively() {
        let config = ReasoningConfig {
            default_preset: Some("same".to_string()),
            presets: vec![
                ReasoningPreset {
                    id: "same".to_string(),
                    setting: Some(ReasoningPresetSetting::Effort {
                        value: "low".to_string(),
                        mode: None,
                    }),
                    ..Default::default()
                },
                ReasoningPreset {
                    id: "same".to_string(),
                    setting: Some(ReasoningPresetSetting::Effort {
                        value: "high".to_string(),
                        mode: None,
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        assert_eq!(
            config.validate_schema(),
            Err("duplicate preset ID 'same'".to_string())
        );
        assert!(matches!(
            config.default_preset().and_then(|preset| preset.setting.as_ref()),
            Some(ReasoningPresetSetting::Effort { value, .. }) if value == "high"
        ));
    }

    #[test]
    fn schema_rejects_non_positive_budget_empty_sequence_and_non_object_patch() {
        assert_eq!(
            config_with(ReasoningPresetSetting::BudgetTokens {
                value: 0,
                mode: None,
            })
            .validate_schema(),
            Err("invalid preset 'custom': budget_tokens value must be greater than 0".to_string())
        );
        assert_eq!(
            config_with(ReasoningPresetSetting::Sequence {
                settings: Vec::new(),
            })
            .validate_schema(),
            Err("invalid preset 'custom': sequence settings must not be empty".to_string())
        );
        assert_eq!(
            config_with(ReasoningPresetSetting::RequestPatch {
                body: json!(["not", "an", "object"]),
            })
            .validate_schema(),
            Err("invalid preset 'custom': request_patch body must be a JSON object".to_string())
        );
    }

    #[test]
    fn schema_recursively_validates_sequence_items() {
        let config = config_with(ReasoningPresetSetting::Sequence {
            settings: vec![ReasoningPresetSetting::Sequence {
                settings: vec![ReasoningPresetSetting::Effort {
                    value: "  ".to_string(),
                    mode: None,
                }],
            }],
        });

        assert_eq!(
            config.validate_schema(),
            Err(
                "invalid preset 'custom': sequence item 0: sequence item 0: effort value must not be empty"
                    .to_string()
            )
        );
    }

    #[test]
    fn schema_accepts_object_request_patch_and_non_empty_sequence() {
        let config = config_with(ReasoningPresetSetting::Sequence {
            settings: vec![
                ReasoningPresetSetting::BudgetTokens {
                    value: 4096,
                    mode: Some(ReasoningMode::Enabled),
                },
                ReasoningPresetSetting::RequestPatch {
                    body: json!({"reasoning": {"effort": "high"}}),
                },
            ],
        });

        assert_eq!(config.validate_schema(), Ok(()));
    }

    #[test]
    fn schema_rejects_empty_catalog_binding_and_enabled_preset_without_setting() {
        let invalid_binding = ReasoningConfig {
            catalog: ReasoningCatalogBinding::ModelsDev {
                provider: "  ".to_string(),
                model: "gpt-test".to_string(),
            },
            ..Default::default()
        };
        assert_eq!(
            invalid_binding.validate_schema(),
            Err("models.dev catalog provider must not be empty".to_string())
        );

        let missing_setting = ReasoningConfig {
            presets: vec![ReasoningPreset {
                id: "custom".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(
            missing_setting.validate_schema(),
            Err("enabled preset 'custom' must define a setting".to_string())
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProxyConfig {
    pub enabled: bool,
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIConfig {
    pub name: String,
    pub base_url: String,
    pub request_url: String,
    pub api_key: String,
    pub model: String,
    pub format: String,
    pub context_window: u32,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub reasoning_mode: ReasoningMode,
    pub inline_think_in_text: bool,
    pub custom_headers: Option<HashMap<String, String>>,
    pub custom_headers_mode: Option<String>,
    pub skip_ssl_verify: bool,
    pub reasoning_effort: Option<String>,
    pub thinking_budget_tokens: Option<u32>,
    pub custom_request_body: Option<Value>,
    pub custom_request_body_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_arguments: Option<String>,
}

impl ToolCall {
    pub fn serialized_arguments(&self) -> String {
        self.raw_arguments
            .as_deref()
            .filter(|raw| serde_json::from_str::<Value>(raw).is_ok())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| {
                serde_json::to_string(&self.arguments).unwrap_or_else(|_| "{}".to_string())
            })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallConfirmationDetails {
    pub request: ToolCallRequestInfo,
    #[serde(rename = "type")]
    pub confirmation_type: String,
    pub message: Option<String>,
    pub file_diff: Option<String>,
    pub file_name: Option<String>,
    pub original_content: Option<String>,
    pub new_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequestInfo {
    pub call_id: String,
    pub name: String,
    pub args: HashMap<String, Value>,
    pub is_client_initiated: bool,
    pub prompt_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResponseInfo {
    pub call_id: String,
    pub response_parts: Value,
    pub result_display: Option<String>,
    pub error: Option<String>,
    pub error_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_image_attachments: Option<Vec<ToolImageAttachment>>,
}

impl Message {
    pub fn user(content: String) -> Self {
        Self {
            role: "user".to_string(),
            content: Some(content),
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: None,
            tool_image_attachments: None,
        }
    }

    pub fn assistant(content: String) -> Self {
        Self {
            role: "assistant".to_string(),
            content: Some(content),
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: None,
            tool_image_attachments: None,
        }
    }

    pub fn assistant_with_tools(tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: None,
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: Some(tool_calls),
            tool_call_id: None,
            name: None,
            is_error: None,
            tool_image_attachments: None,
        }
    }

    pub fn system(content: String) -> Self {
        Self {
            role: "system".to_string(),
            content: Some(content),
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: None,
            tool_image_attachments: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionTestMessageCode {
    ToolCallsNotDetected,
    ImageInputCheckFailed,
    TlsOrCertificateIssue,
    ProxyIssue,
    NetworkIssue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionTestResult {
    pub success: bool,
    pub response_time_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_response: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_code: Option<ConnectionTestMessageCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteModelInfo {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}
