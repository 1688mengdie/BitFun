//! models.dev parsing, provider/model matching, and reasoning preset projection.

use bitfun_core_types::{
    ReasoningCapabilityStatus, ReasoningCatalogBinding, ReasoningCatalogProjection,
    ReasoningConfig, ReasoningMode, ReasoningPresetDescriptor, ReasoningPresetSetting,
    ReasoningPresetSource,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

use crate::client::quirks::is_deepseek_reasoning_effort_model;
use crate::providers::anthropic::request::{
    anthropic_thinking_capability, AnthropicThinkingCapability,
};
use crate::providers::openai::common::is_known_codex_reasoning_model;

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
                    ModelsDevReasoningOption::Effort { values } if support.effort => {
                        effort_descriptors(
                            values.iter().map(String::as_str),
                            support.effort_mode,
                            ReasoningPresetSource::ModelsDev,
                        )
                    }
                    ModelsDevReasoningOption::Toggle if support.toggle => {
                        toggle_descriptors(ReasoningPresetSource::ModelsDev)
                    }
                    ModelsDevReasoningOption::BudgetTokens { min, max }
                        if support.budget_tokens =>
                    {
                        budget_descriptors(*min, *max, ReasoningPresetSource::ModelsDev)
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

    // models.dev remains authoritative when it explicitly says the model is
    // not reasoning-capable. Otherwise, a tested adapter fallback fills gaps
    // in a missing or incomplete snapshot. Fallbacks are available only for
    // auto-bound official endpoints; an explicit models.dev binding may point
    // at an arbitrary gateway and must not grant adapter-inferred capability.
    if matches!(binding, ReasoningCatalogBinding::Auto)
        && source_model.is_none_or(|model| model.reasoning)
    {
        for descriptor in adapter_fallback_descriptors(provider, model_id, base_url, support) {
            if source_model.is_some_and(|model| {
                model
                    .reasoning_options
                    .iter()
                    .any(|option| models_dev_option_covers_setting(option, &descriptor.setting))
            }) {
                continue;
            }
            descriptors
                .entry(descriptor.id.clone())
                .or_insert(descriptor);
        }
    }

    if let Some(config) = configured {
        for preset in &config.presets {
            let preset_id = preset.id.trim();
            if preset_id.is_empty() {
                continue;
            }
            if preset.disabled {
                descriptors.remove(preset_id);
                continue;
            }
            let Some(setting) = preset.setting.clone() else {
                descriptors.remove(preset_id);
                continue;
            };
            descriptors.insert(
                preset_id.to_string(),
                ReasoningPresetDescriptor {
                    id: preset_id.to_string(),
                    label: preset
                        .label
                        .clone()
                        .filter(|label| !label.trim().is_empty())
                        .unwrap_or_else(|| display_label(preset_id)),
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
        .and_then(|config| config.default_preset.as_deref())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .filter(|id| presets.iter().any(|preset| preset.id == *id));

    ReasoningCatalogProjection {
        status,
        default_preset: default_preset.map(ToOwned::to_owned),
        presets,
    }
}

fn auto_provider_id(provider: &str, base_url: &str) -> Option<&'static str> {
    let provider = provider.trim().to_ascii_lowercase();
    let endpoint = reqwest::Url::parse(base_url.trim()).ok()?;
    if endpoint.scheme() != "https" || endpoint.port_or_known_default() != Some(443) {
        return None;
    }
    let host = endpoint.host_str()?.trim_end_matches('.');
    match (provider.as_str(), host) {
        ("response" | "responses" | "openai", "api.openai.com") => Some("openai"),
        ("anthropic", "api.anthropic.com") => Some("anthropic"),
        ("gemini" | "google", "generativelanguage.googleapis.com") => Some("google"),
        ("deepseek" | "openai" | "anthropic", "api.deepseek.com") => Some("deepseek"),
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
            toggle: true,
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

fn is_codex_chatgpt_path(path: &str) -> bool {
    let path = path.trim_end_matches('/');
    path == "/backend-api/codex" || path == "/backend-api/codex/responses"
}

fn adapter_fallback_descriptors(
    provider: &str,
    model_id: &str,
    base_url: &str,
    support: AdapterReasoningSupport,
) -> Vec<ReasoningPresetDescriptor> {
    let Some(provider_id) = adapter_fallback_provider_id(provider, base_url) else {
        return Vec::new();
    };
    let model_id = model_id.trim().to_ascii_lowercase();
    let source = ReasoningPresetSource::AdapterFallback;

    match provider_id {
        // Keep these tables deliberately conservative. A future model is not
        // assumed compatible merely because the protocol has an effort field.
        "openai" if support.effort => {
            if is_codex_chatgpt_base_url(base_url) && is_known_codex_reasoning_model(&model_id) {
                // The Codex adapter clamps unsupported `minimal` and uses
                // medium by default. low/medium/high is the tested common set
                // across its built-in model table.
                effort_descriptors(["low", "medium", "high"], None, source)
            } else {
                match model_id.as_str() {
                    "gpt-5.4" => {
                        effort_descriptors(["none", "low", "medium", "high", "xhigh"], None, source)
                    }
                    "gpt-5.2-pro" => effort_descriptors(["medium", "high", "xhigh"], None, source),
                    _ => Vec::new(),
                }
            }
        }
        "anthropic" => match anthropic_thinking_capability(&model_id) {
            AnthropicThinkingCapability::AdaptivePreferred
            | AnthropicThinkingCapability::AdaptiveOnly
            | AnthropicThinkingCapability::AdaptiveDefaultNoDisabled
                if support.effort =>
            {
                // low/medium/high is the conservative common subset for the
                // adaptive families recognized by the request adapter. More
                // model-specific values such as max/xhigh remain models.dev
                // facts and are not inferred here.
                effort_descriptors(
                    ["low", "medium", "high"],
                    Some(ReasoningMode::Adaptive),
                    source,
                )
            }
            // These exact models are covered by the adapter's manual-thinking
            // request tests or built-in model list. `ManualOnly` is otherwise
            // the unknown/default classification, so it must never become a
            // family-wide fallback. Budget choices are derived from max_tokens
            // at request time, so the catalog exposes only a safe on/off mode.
            AnthropicThinkingCapability::ManualOnly
                if matches!(model_id.as_str(), "claude-sonnet-4-5" | "claude-haiku-4-5")
                    && support.toggle =>
            {
                toggle_descriptors(source)
            }
            _ => Vec::new(),
        },
        "deepseek"
            if is_deepseek_reasoning_effort_model(&model_id)
                && support.effort
                && support.toggle =>
        {
            let mut descriptors = toggle_descriptors(source);
            descriptors.extend(effort_descriptors(
                ["high", "max"],
                Some(ReasoningMode::Enabled),
                source,
            ));
            descriptors
        }
        // Gemini can serialize the current mode, but the adapter does not yet
        // own a tested model-level table for whether thinking can be disabled
        // or which budgets/levels are accepted. Keep it fail closed here.
        _ => Vec::new(),
    }
}

fn adapter_fallback_provider_id(provider: &str, base_url: &str) -> Option<&'static str> {
    if let Some(provider_id) = auto_provider_id(provider, base_url) {
        return Some(provider_id);
    }

    let provider = provider.trim().to_ascii_lowercase();
    let endpoint = reqwest::Url::parse(base_url.trim()).ok()?;
    if endpoint.scheme() != "https" || endpoint.port_or_known_default() != Some(443) {
        return None;
    }
    let host = endpoint.host_str()?.trim_end_matches('.');
    match (provider.as_str(), host) {
        ("response" | "responses", "chatgpt.com") if is_codex_chatgpt_path(endpoint.path()) => {
            Some("openai")
        }
        _ => None,
    }
}

fn is_codex_chatgpt_base_url(base_url: &str) -> bool {
    reqwest::Url::parse(base_url.trim())
        .ok()
        .is_some_and(|url| {
            url.scheme() == "https"
                && url.port_or_known_default() == Some(443)
                && url
                    .host_str()
                    .is_some_and(|host| host.trim_end_matches('.') == "chatgpt.com")
                && is_codex_chatgpt_path(url.path())
        })
}

fn models_dev_option_covers_setting(
    option: &ModelsDevReasoningOption,
    setting: &ReasoningPresetSetting,
) -> bool {
    match (option, setting) {
        (ModelsDevReasoningOption::Effort { values }, ReasoningPresetSetting::Effort { .. }) => {
            !values.is_empty()
        }
        (ModelsDevReasoningOption::Toggle, ReasoningPresetSetting::Toggle { .. }) => true,
        (
            ModelsDevReasoningOption::BudgetTokens { .. },
            ReasoningPresetSetting::BudgetTokens { .. },
        ) => true,
        _ => false,
    }
}

fn effort_descriptors<'a>(
    values: impl IntoIterator<Item = &'a str>,
    mode: Option<ReasoningMode>,
    source: ReasoningPresetSource,
) -> Vec<ReasoningPresetDescriptor> {
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| ReasoningPresetDescriptor {
            id: value.to_string(),
            label: display_label(value),
            order: 10 + index as i32,
            setting: ReasoningPresetSetting::Effort {
                value: value.to_string(),
                mode,
            },
            source,
        })
        .collect()
}

fn toggle_descriptors(source: ReasoningPresetSource) -> Vec<ReasoningPresetDescriptor> {
    vec![
        ReasoningPresetDescriptor {
            id: "off".to_string(),
            label: "Off".to_string(),
            order: 0,
            setting: ReasoningPresetSetting::Toggle { enabled: false },
            source,
        },
        ReasoningPresetDescriptor {
            id: "on".to_string(),
            label: "On".to_string(),
            order: 1,
            setting: ReasoningPresetSetting::Toggle { enabled: true },
            source,
        },
    ]
}

fn budget_descriptors(
    min: Option<u32>,
    max: Option<u32>,
    source: ReasoningPresetSource,
) -> Vec<ReasoningPresetDescriptor> {
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
            source,
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
        ReasoningPresetSetting, ReasoningPresetSource,
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
                }},
                "google": {"models": {
                    "gemini-test": {"id":"gemini-test","reasoning":true,
                        "reasoning_options":{"type":"toggle"}}
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
    fn tested_anthropic_family_uses_adapter_fallback_without_a_snapshot_model() {
        let projection = project_reasoning_catalog(
            "anthropic",
            "claude-opus-4-8",
            "https://api.anthropic.com/v1/messages",
            None,
            Some(&catalog()),
        );

        assert_eq!(projection.status, ReasoningCapabilityStatus::Known);
        assert_eq!(
            projection
                .presets
                .iter()
                .map(|preset| (preset.id.as_str(), preset.source))
                .collect::<Vec<_>>(),
            [
                ("low", ReasoningPresetSource::AdapterFallback),
                ("medium", ReasoningPresetSource::AdapterFallback),
                ("high", ReasoningPresetSource::AdapterFallback),
            ]
        );
    }

    #[test]
    fn tested_manual_anthropic_model_uses_conservative_toggle_fallback() {
        let projection = project_reasoning_catalog(
            "anthropic",
            "claude-haiku-4-5",
            "https://api.anthropic.com/v1/messages",
            None,
            None,
        );

        assert_eq!(projection.status, ReasoningCapabilityStatus::Known);
        assert_eq!(
            projection
                .presets
                .iter()
                .map(|preset| preset.id.as_str())
                .collect::<Vec<_>>(),
            ["off", "on"]
        );
        assert!(projection
            .presets
            .iter()
            .all(|preset| preset.source == ReasoningPresetSource::AdapterFallback));
    }

    #[test]
    fn codex_builtin_model_uses_adapter_fallback_without_a_snapshot_model() {
        let projection = project_reasoning_catalog(
            "responses",
            "gpt-5.5",
            "https://chatgpt.com/backend-api/codex",
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
            ["low", "medium", "high"]
        );
        assert!(projection
            .presets
            .iter()
            .all(|preset| preset.source == ReasoningPresetSource::AdapterFallback));
    }

    #[test]
    fn codex_endpoint_does_not_auto_bind_public_openai_catalog_records() {
        let public_openai = ModelsDevCatalog::parse_str(
            r#"{
                "openai": {"models": {
                    "gpt-5.5": {
                        "id":"gpt-5.5",
                        "reasoning":true,
                        "reasoning_options":{"type":"effort","values":["xhigh"]}
                    }
                }}
            }"#,
        )
        .expect("catalog should parse");
        let projection = project_reasoning_catalog(
            "responses",
            "gpt-5.5",
            "https://chatgpt.com/backend-api/codex/responses",
            None,
            Some(&public_openai),
        );

        assert_eq!(
            projection
                .presets
                .iter()
                .map(|preset| preset.id.as_str())
                .collect::<Vec<_>>(),
            ["low", "medium", "high"]
        );
        assert!(projection
            .presets
            .iter()
            .all(|preset| preset.source == ReasoningPresetSource::AdapterFallback));
    }

    #[test]
    fn deepseek_exact_model_uses_adapter_fallback_when_catalog_is_unavailable() {
        let projection = project_reasoning_catalog(
            "deepseek",
            "deepseek-v4-pro",
            "https://api.deepseek.com/v1",
            None,
            None,
        );

        assert_eq!(projection.status, ReasoningCapabilityStatus::Known);
        assert_eq!(
            projection
                .presets
                .iter()
                .map(|preset| preset.id.as_str())
                .collect::<Vec<_>>(),
            ["off", "on", "high", "max"]
        );
        assert!(projection
            .presets
            .iter()
            .all(|preset| preset.source == ReasoningPresetSource::AdapterFallback));
    }

    #[test]
    fn fallback_fills_missing_option_types_and_model_config_still_wins() {
        let partial = ModelsDevCatalog::parse_str(
            r#"{
                "anthropic": {"models": {
                    "claude-opus-4-8": {
                        "id":"claude-opus-4-8",
                        "reasoning":true,
                        "reasoning_options":{"type":"budget_tokens","min":2048}
                    }
                }}
            }"#,
        )
        .expect("partial catalog should parse");
        let configured = ReasoningConfig {
            default_preset: Some("high".to_string()),
            presets: vec![ReasoningPreset {
                id: "high".to_string(),
                label: Some("Configured high".to_string()),
                setting: Some(ReasoningPresetSetting::Effort {
                    value: "max".to_string(),
                    mode: Some(bitfun_core_types::ReasoningMode::Adaptive),
                }),
                ..Default::default()
            }],
            ..Default::default()
        };

        let projection = project_reasoning_catalog(
            "anthropic",
            "claude-opus-4-8",
            "https://api.anthropic.com/v1/messages",
            Some(&configured),
            Some(&partial),
        );

        assert_eq!(projection.default_preset.as_deref(), Some("high"));
        assert_eq!(
            projection
                .presets
                .iter()
                .find(|preset| preset.id == "budget")
                .expect("models.dev budget")
                .source,
            ReasoningPresetSource::ModelsDev
        );
        assert_eq!(
            projection
                .presets
                .iter()
                .find(|preset| preset.id == "low")
                .expect("adapter fallback effort")
                .source,
            ReasoningPresetSource::AdapterFallback
        );
        let high = projection
            .presets
            .iter()
            .find(|preset| preset.id == "high")
            .expect("configured high");
        assert_eq!(high.label, "Configured high");
        assert_eq!(high.source, ReasoningPresetSource::ModelConfig);
    }

    #[test]
    fn explicit_non_reasoning_catalog_fact_blocks_adapter_fallback() {
        let non_reasoning = ModelsDevCatalog::parse_str(
            r#"{
                "openai": {"models": {
                    "gpt-5.4": {"id":"gpt-5.4","reasoning":false}
                }}
            }"#,
        )
        .expect("catalog should parse");
        let projection = project_reasoning_catalog(
            "responses",
            "gpt-5.4",
            "https://api.openai.com/v1/responses",
            None,
            Some(&non_reasoning),
        );

        assert_eq!(projection.status, ReasoningCapabilityStatus::Unsupported);
        assert!(projection.presets.is_empty());
    }

    #[test]
    fn unknown_official_model_stays_fail_closed() {
        let projection = project_reasoning_catalog(
            "responses",
            "gpt-9-unknown",
            "https://api.openai.com/v1/responses",
            None,
            None,
        );

        assert_eq!(projection.status, ReasoningCapabilityStatus::Unknown);
        assert!(projection.presets.is_empty());
    }

    #[test]
    fn explicit_models_dev_binding_does_not_enable_adapter_fallback_on_a_gateway() {
        let configured = ReasoningConfig {
            catalog: ReasoningCatalogBinding::ModelsDev {
                provider: "anthropic".to_string(),
                model: "claude-opus-4-8".to_string(),
            },
            ..Default::default()
        };
        let projection = project_reasoning_catalog(
            "anthropic",
            "gateway-alias",
            "https://gateway.example.com/v1/messages",
            Some(&configured),
            None,
        );

        assert_eq!(projection.status, ReasoningCapabilityStatus::Unknown);
        assert!(projection.presets.is_empty());
    }

    #[test]
    fn auto_catalog_rejects_custom_and_untrusted_endpoints() {
        for (provider, model, base_url) in [
            (
                "responses",
                "gpt-test",
                "https://gateway.example.com/v1/responses",
            ),
            (
                "anthropic",
                "claude-test",
                "https://gateway.example.com/anthropic",
            ),
            (
                "gemini",
                "gemini-test",
                "https://gateway.example.com/gemini",
            ),
            (
                "openai",
                "deepseek-v4-flash",
                "https://api.deepseek.com.evil.example/v1",
            ),
            (
                "responses",
                "gpt-test",
                "http://api.openai.com/v1/responses",
            ),
            (
                "responses",
                "gpt-test",
                "https://api.openai.com:8443/v1/responses",
            ),
            (
                "responses",
                "gpt-5.5",
                "https://chatgpt.com.evil.example/backend-api/codex",
            ),
            (
                "responses",
                "gpt-5.5",
                "https://chatgpt.com:8443/backend-api/codex",
            ),
            (
                "anthropic",
                "claude-opus-4-8",
                "https://gateway.example.com/v1/messages",
            ),
        ] {
            let projection =
                project_reasoning_catalog(provider, model, base_url, None, Some(&catalog()));
            assert_eq!(
                projection.status,
                ReasoningCapabilityStatus::Unknown,
                "auto catalog must fail closed for {base_url}"
            );
            assert!(projection.presets.is_empty());
        }
    }

    #[test]
    fn auto_catalog_requires_the_official_endpoint_to_match_the_protocol() {
        let projection = project_reasoning_catalog(
            "anthropic",
            "gpt-test",
            "https://api.openai.com/v1",
            None,
            Some(&catalog()),
        );

        assert_eq!(projection.status, ReasoningCapabilityStatus::Unknown);
        assert!(projection.presets.is_empty());
    }

    #[test]
    fn explicit_models_dev_binding_allows_a_custom_endpoint() {
        let configured = ReasoningConfig {
            catalog: ReasoningCatalogBinding::ModelsDev {
                provider: "openai".to_string(),
                model: "gpt-test".to_string(),
            },
            ..Default::default()
        };
        let projection = project_reasoning_catalog(
            "responses",
            "gateway-model-alias",
            "https://gateway.example.com/v1/responses",
            Some(&configured),
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
    fn official_google_endpoint_projects_gemini_options() {
        let projection = project_reasoning_catalog(
            "gemini",
            "gemini-test",
            "https://generativelanguage.googleapis.com/v1beta",
            None,
            Some(&catalog()),
        );

        assert_eq!(projection.status, ReasoningCapabilityStatus::Known);
        assert!(projection.presets.iter().any(|preset| preset.id == "on"));
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

    #[test]
    fn model_config_can_hide_an_adapter_fallback_preset() {
        let configured = ReasoningConfig {
            presets: vec![ReasoningPreset {
                id: "medium".to_string(),
                disabled: true,
                ..Default::default()
            }],
            ..Default::default()
        };
        let projection = project_reasoning_catalog(
            "responses",
            "gpt-5.5",
            "https://chatgpt.com/backend-api/codex/responses",
            Some(&configured),
            None,
        );

        assert!(projection.presets.iter().any(|preset| preset.id == "low"));
        assert!(!projection
            .presets
            .iter()
            .any(|preset| preset.id == "medium"));
    }

    #[test]
    fn later_duplicate_without_setting_removes_the_earlier_descriptor() {
        let configured = ReasoningConfig {
            catalog: ReasoningCatalogBinding::Disabled,
            default_preset: Some("same".to_string()),
            presets: vec![
                ReasoningPreset {
                    id: "same".to_string(),
                    setting: Some(ReasoningPresetSetting::Toggle { enabled: true }),
                    ..Default::default()
                },
                ReasoningPreset {
                    id: "same".to_string(),
                    setting: None,
                    ..Default::default()
                },
            ],
        };

        let projection = project_reasoning_catalog(
            "responses",
            "gpt-test",
            "https://api.openai.com/v1/responses",
            Some(&configured),
            Some(&catalog()),
        );

        assert!(projection.presets.is_empty());
        assert!(projection.default_preset.is_none());
    }
}
