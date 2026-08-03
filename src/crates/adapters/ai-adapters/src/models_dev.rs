//! models.dev parsing, provider/model matching, and reasoning preset projection.

use bitfun_core_types::{
    ReasoningCapabilityStatus, ReasoningCatalogBinding, ReasoningCatalogProjection,
    ReasoningConfig, ReasoningMode, ReasoningPresetDescriptor, ReasoningPresetSetting,
    ReasoningPresetSource,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, PartialEq)]
pub struct ModelsDevCatalog {
    providers: BTreeMap<String, ModelsDevProvider>,
}

#[derive(Debug, Clone, PartialEq)]
struct ModelsDevProvider {
    models: BTreeMap<String, ModelsDevModel>,
}

#[derive(Debug, Clone, PartialEq)]
struct ModelsDevModel {
    id: String,
    reasoning: bool,
    reasoning_options: Vec<ModelsDevReasoningOption>,
}

#[derive(Debug, Clone, PartialEq)]
enum ModelsDevReasoningOption {
    Effort { values: Vec<String> },
    Toggle,
    BudgetTokens { min: Option<u32>, max: Option<u32> },
}

#[derive(Debug, Deserialize, Default)]
struct RawProvider {
    #[serde(default)]
    models: HashMap<String, RawModel>,
}

#[derive(Debug, Deserialize, Default)]
struct RawModel {
    #[serde(default)]
    id: String,
    #[serde(default)]
    reasoning: bool,
    #[serde(default, deserialize_with = "deserialize_reasoning_options")]
    reasoning_options: Vec<RawReasoningOption>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RawReasoningOption {
    Effort {
        #[serde(default)]
        values: Vec<Option<String>>,
    },
    Toggle,
    BudgetTokens {
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
}

fn deserialize_reasoning_options<'de, D>(
    deserializer: D,
) -> Result<Vec<RawReasoningOption>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?.unwrap_or(Value::Null);
    let values = match value {
        Value::Array(values) => values,
        Value::Object(_) => vec![value],
        _ => Vec::new(),
    };
    Ok(values
        .into_iter()
        .filter_map(|value| serde_json::from_value(value).ok())
        .collect())
}

impl ModelsDevCatalog {
    pub fn parse_str(body: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(body)
            .map_err(|error| format!("models.dev catalog JSON is invalid: {error}"))?;
        Self::from_value(value)
    }

    pub fn from_value(value: Value) -> Result<Self, String> {
        let providers = value
            .as_object()
            .ok_or_else(|| "models.dev catalog must be a provider object".to_string())?;
        let mut parsed = BTreeMap::new();
        for (provider_id, provider_value) in providers {
            let provider_id = provider_id.trim().to_ascii_lowercase();
            if provider_id.is_empty() {
                continue;
            }
            let Ok(raw_provider) = serde_json::from_value::<RawProvider>(provider_value.clone())
            else {
                continue;
            };
            let mut models = BTreeMap::new();
            for (model_key, raw_model) in raw_provider.models {
                let model_id = if raw_model.id.trim().is_empty() {
                    model_key.clone()
                } else {
                    raw_model.id
                };
                if model_id.trim().is_empty() {
                    continue;
                }
                models.insert(
                    model_key,
                    ModelsDevModel {
                        id: model_id,
                        reasoning: raw_model.reasoning,
                        reasoning_options: raw_model
                            .reasoning_options
                            .into_iter()
                            .filter_map(|option| match option {
                                RawReasoningOption::Effort { values } => {
                                    Some(ModelsDevReasoningOption::Effort {
                                        values: values
                                            .into_iter()
                                            .flatten()
                                            .map(|value| value.trim().to_string())
                                            .filter(|value| !value.is_empty())
                                            .collect(),
                                    })
                                }
                                RawReasoningOption::Toggle => {
                                    Some(ModelsDevReasoningOption::Toggle)
                                }
                                RawReasoningOption::BudgetTokens { min, max } => {
                                    Some(ModelsDevReasoningOption::BudgetTokens { min, max })
                                }
                            })
                            .collect(),
                    },
                );
            }
            if !models.is_empty() {
                parsed.insert(provider_id, ModelsDevProvider { models });
            }
        }
        Ok(Self { providers: parsed })
    }

    fn model(&self, provider_id: &str, model_id: &str) -> Option<&ModelsDevModel> {
        let provider_id = provider_id.trim().to_ascii_lowercase();
        self.providers
            .get(&provider_id)
            .and_then(|provider| provider.models.get(model_id))
            .or_else(|| {
                self.providers.get(&provider_id).and_then(|provider| {
                    provider.models.values().find(|model| model.id == model_id)
                })
            })
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct AdapterReasoningSupport {
    effort: bool,
    effort_mode: Option<ReasoningMode>,
    toggle: bool,
    budget_tokens: bool,
}

/// Project a source catalog and user-configured presets into the stable DTO
/// consumed by remote and Web UI surfaces.
pub fn project_reasoning_catalog(
    provider: &str,
    model_id: &str,
    base_url: &str,
    configured: Option<&ReasoningConfig>,
    models_dev: Option<&ModelsDevCatalog>,
) -> ReasoningCatalogProjection {
    let binding = configured
        .map(|config| &config.catalog)
        .cloned()
        .unwrap_or_default();
    let support = adapter_reasoning_support(provider, base_url);
    let source_model = match &binding {
        ReasoningCatalogBinding::Disabled => None,
        ReasoningCatalogBinding::Auto => models_dev
            .and_then(|catalog| catalog.model(auto_provider_id(provider, base_url)?, model_id)),
        ReasoningCatalogBinding::ModelsDev { provider, model } => {
            models_dev.and_then(|catalog| catalog.model(provider, model))
        }
    };

    let mut descriptors = BTreeMap::<String, ReasoningPresetDescriptor>::new();
    let mut has_unmapped_reasoning = false;
    if let Some(source_model) = source_model {
        if source_model.reasoning {
            for option in &source_model.reasoning_options {
                let generated = match option {
                    ModelsDevReasoningOption::Effort { values } if support.effort => values
                        .iter()
                        .enumerate()
                        .map(|(index, value)| ReasoningPresetDescriptor {
                            id: value.clone(),
                            label: display_label(value),
                            order: 10 + index as i32,
                            setting: ReasoningPresetSetting::Effort {
                                value: value.clone(),
                                mode: support.effort_mode,
                            },
                            source: ReasoningPresetSource::ModelsDev,
                        })
                        .collect::<Vec<_>>(),
                    ModelsDevReasoningOption::Toggle if support.toggle => vec![
                        ReasoningPresetDescriptor {
                            id: "off".to_string(),
                            label: "Off".to_string(),
                            order: 0,
                            setting: ReasoningPresetSetting::Toggle { enabled: false },
                            source: ReasoningPresetSource::ModelsDev,
                        },
                        ReasoningPresetDescriptor {
                            id: "on".to_string(),
                            label: "On".to_string(),
                            order: 1,
                            setting: ReasoningPresetSetting::Toggle { enabled: true },
                            source: ReasoningPresetSource::ModelsDev,
                        },
                    ],
                    ModelsDevReasoningOption::BudgetTokens { min, max }
                        if support.budget_tokens =>
                    {
                        budget_descriptors(*min, *max)
                    }
                    ModelsDevReasoningOption::Effort { .. }
                    | ModelsDevReasoningOption::Toggle
                    | ModelsDevReasoningOption::BudgetTokens { .. } => {
                        has_unmapped_reasoning = true;
                        Vec::new()
                    }
                };
                for descriptor in generated {
                    descriptors.insert(descriptor.id.clone(), descriptor);
                }
            }
            if source_model.reasoning_options.is_empty() {
                has_unmapped_reasoning = true;
            }
        }
    }

    if let Some(config) = configured {
        for preset in &config.presets {
            if preset.disabled {
                descriptors.remove(&preset.id);
                continue;
            }
            let Some(setting) = preset.setting.clone() else {
                continue;
            };
            descriptors.insert(
                preset.id.clone(),
                ReasoningPresetDescriptor {
                    id: preset.id.clone(),
                    label: preset
                        .label
                        .clone()
                        .filter(|label| !label.trim().is_empty())
                        .unwrap_or_else(|| display_label(&preset.id)),
                    order: preset.order.unwrap_or(100),
                    setting,
                    source: ReasoningPresetSource::ModelConfig,
                },
            );
        }
    }

    let mut presets = descriptors.into_values().collect::<Vec<_>>();
    presets.sort_by(|left, right| {
        left.order
            .cmp(&right.order)
            .then_with(|| left.id.cmp(&right.id))
    });
    let status = if !presets.is_empty() {
        ReasoningCapabilityStatus::Known
    } else if matches!(binding, ReasoningCatalogBinding::Disabled) {
        ReasoningCapabilityStatus::Unsupported
    } else if has_unmapped_reasoning {
        ReasoningCapabilityStatus::Unknown
    } else if source_model.is_some() {
        ReasoningCapabilityStatus::Unsupported
    } else {
        ReasoningCapabilityStatus::Unknown
    };
    let default_preset = configured
        .and_then(|config| config.default_preset.clone())
        .filter(|id| presets.iter().any(|preset| preset.id == *id));

    ReasoningCatalogProjection {
        status,
        default_preset,
        presets,
    }
}

fn auto_provider_id(provider: &str, base_url: &str) -> Option<&'static str> {
    let provider = provider.trim().to_ascii_lowercase();
    if provider == "deepseek" || base_url.to_ascii_lowercase().contains("api.deepseek.com") {
        return Some("deepseek");
    }
    match provider.as_str() {
        "response" | "responses" | "openai" => Some("openai"),
        "anthropic" => Some("anthropic"),
        "gemini" | "google" => Some("google"),
        _ => None,
    }
}

fn adapter_reasoning_support(provider: &str, base_url: &str) -> AdapterReasoningSupport {
    let provider = provider.trim().to_ascii_lowercase();
    if provider == "deepseek" || base_url.to_ascii_lowercase().contains("api.deepseek.com") {
        return AdapterReasoningSupport {
            effort: true,
            toggle: true,
            ..Default::default()
        };
    }
    if matches!(provider.as_str(), "response" | "responses")
        || (provider == "openai" && is_responses_endpoint(base_url))
    {
        return AdapterReasoningSupport {
            effort: true,
            ..Default::default()
        };
    }
    match provider.as_str() {
        "anthropic" => AdapterReasoningSupport {
            effort: true,
            effort_mode: Some(ReasoningMode::Adaptive),
            budget_tokens: true,
            ..Default::default()
        },
        "gemini" | "google" => AdapterReasoningSupport {
            toggle: true,
            ..Default::default()
        },
        _ => Default::default(),
    }
}

fn is_responses_endpoint(base_url: &str) -> bool {
    base_url
        .trim_end_matches('/')
        .to_ascii_lowercase()
        .ends_with("/responses")
}

fn budget_descriptors(min: Option<u32>, max: Option<u32>) -> Vec<ReasoningPresetDescriptor> {
    let min = min.or(max).unwrap_or(1024);
    let mut values = vec![("budget", min)];
    if let Some(max) = max.filter(|max| *max != min) {
        values.push(("budget-max", max));
    }
    values
        .into_iter()
        .enumerate()
        .map(|(index, (id, value))| ReasoningPresetDescriptor {
            id: id.to_string(),
            label: if id == "budget" {
                "Budget".to_string()
            } else {
                "Budget max".to_string()
            },
            order: 30 + index as i32,
            setting: ReasoningPresetSetting::BudgetTokens { value, mode: None },
            source: ReasoningPresetSource::ModelsDev,
        })
        .collect()
}

fn display_label(value: &str) -> String {
    let mut chars = value.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{project_reasoning_catalog, ModelsDevCatalog};
    use bitfun_core_types::{
        ReasoningCapabilityStatus, ReasoningCatalogBinding, ReasoningConfig, ReasoningPreset,
        ReasoningPresetSetting,
    };

    fn catalog() -> ModelsDevCatalog {
        ModelsDevCatalog::parse_str(
            r#"{
                "openai": {"models": {
                    "gpt-test": {"id":"gpt-test","reasoning":true,
                        "reasoning_options":{"type":"effort","values":["low","high"]}}
                }},
                "anthropic": {"models": {
                    "claude-test": {"id":"claude-test","reasoning":true,
                        "reasoning_options":[{"type":"effort","values":["low","high"]},{"type":"budget_tokens","min":1024}]}
                }},
                "deepseek": {"models": {
                    "deepseek-v4-flash": {"id":"deepseek-v4-flash","reasoning":true,
                        "reasoning_options":[{"type":"toggle"},{"type":"effort","values":["high","max"]}]}
                }}
            }"#,
        )
        .expect("catalog should parse")
    }

    #[test]
    fn responses_effort_options_are_projected_as_known_presets() {
        let projection = project_reasoning_catalog(
            "responses",
            "gpt-test",
            "https://api.openai.com/v1/responses",
            None,
            Some(&catalog()),
        );

        assert_eq!(projection.status, ReasoningCapabilityStatus::Known);
        assert_eq!(
            projection
                .presets
                .iter()
                .map(|preset| preset.id.as_str())
                .collect::<Vec<_>>(),
            ["low", "high"]
        );
    }

    #[test]
    fn anthropic_budget_and_effort_options_are_merged() {
        let projection = project_reasoning_catalog(
            "anthropic",
            "claude-test",
            "https://api.anthropic.com/v1/messages",
            None,
            Some(&catalog()),
        );

        assert_eq!(projection.status, ReasoningCapabilityStatus::Known);
        assert!(projection
            .presets
            .iter()
            .any(|preset| preset.id == "budget"));
        assert!(projection.presets.iter().any(|preset| {
            preset.id == "high"
                && matches!(
                    preset.setting,
                    ReasoningPresetSetting::Effort {
                        mode: Some(bitfun_core_types::ReasoningMode::Adaptive),
                        ..
                    }
                )
        }));
    }

    #[test]
    fn deepseek_toggle_and_effort_options_are_projected() {
        let projection = project_reasoning_catalog(
            "openai",
            "deepseek-v4-flash",
            "https://api.deepseek.com/v1",
            None,
            Some(&catalog()),
        );

        assert_eq!(projection.status, ReasoningCapabilityStatus::Known);
        assert!(projection.presets.iter().any(|preset| preset.id == "off"));
        assert!(projection.presets.iter().any(|preset| preset.id == "max"));
    }

    #[test]
    fn unsupported_effort_mapping_is_unknown_and_custom_presets_remain_available() {
        let configured = ReasoningConfig {
            catalog: ReasoningCatalogBinding::Auto,
            default_preset: Some("custom".to_string()),
            presets: vec![ReasoningPreset {
                id: "custom".to_string(),
                label: Some("Custom".to_string()),
                order: None,
                disabled: false,
                setting: Some(ReasoningPresetSetting::RequestPatch {
                    body: serde_json::json!({"thinking": {"type": "enabled"}}),
                }),
            }],
        };
        let projection = project_reasoning_catalog(
            "openai",
            "gpt-test",
            "https://example.com/v1/chat/completions",
            Some(&configured),
            Some(&catalog()),
        );

        assert_eq!(projection.status, ReasoningCapabilityStatus::Known);
        assert_eq!(projection.default_preset.as_deref(), Some("custom"));
        assert_eq!(projection.presets.len(), 1);
    }

    #[test]
    fn unsupported_effort_mapping_without_custom_presets_is_unknown() {
        let projection = project_reasoning_catalog(
            "openai",
            "gpt-test",
            "https://example.com/v1/chat/completions",
            None,
            Some(&catalog()),
        );

        assert_eq!(projection.status, ReasoningCapabilityStatus::Unknown);
        assert!(projection.presets.is_empty());
    }

    #[test]
    fn disabled_catalog_binding_hides_generated_options() {
        let configured = ReasoningConfig {
            catalog: ReasoningCatalogBinding::Disabled,
            ..Default::default()
        };
        let projection = project_reasoning_catalog(
            "responses",
            "gpt-test",
            "https://api.openai.com/v1/responses",
            Some(&configured),
            Some(&catalog()),
        );

        assert_eq!(projection.status, ReasoningCapabilityStatus::Unsupported);
        assert!(projection.presets.is_empty());
    }
}
