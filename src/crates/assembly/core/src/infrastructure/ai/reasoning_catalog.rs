use std::sync::Arc;
#[cfg(feature = "product-full")]
use std::sync::{OnceLock, RwLock};
#[cfg(feature = "product-full")]
use std::time::{Duration, Instant};

use bitfun_ai_adapters::models_dev::{project_reasoning_catalog, ModelsDevCatalog};
use bitfun_core_types::{
    ReasoningCatalogProjection, ReasoningPresetDescriptor, ReasoningPresetSetting,
};
#[cfg(feature = "product-full")]
use bitfun_events::{AIModelCatalogUpdatedEvent, AI_MODEL_CATALOG_UPDATED_EVENT};
#[cfg(feature = "product-full")]
use bitfun_services_integrations::models_dev::{
    ModelsDevCatalogService, ModelsDevRefreshOutcome, ModelsDevSnapshot,
};
#[cfg(feature = "product-full")]
use log::debug;

use crate::infrastructure::ai::AIClient;
use crate::service::config::types::AIModelConfig;

#[derive(Clone)]
pub(crate) struct ModelsDevReasoningCatalogSnapshot {
    pub(crate) catalog: Option<Arc<ModelsDevCatalog>>,
    #[cfg(feature = "product-full")]
    pub(crate) version: u64,
}

#[cfg(feature = "product-full")]
const CATALOG_RELOAD_INTERVAL: Duration = Duration::from_secs(60);

#[cfg(feature = "product-full")]
struct CachedReasoningCatalogSnapshot {
    loaded_at: Instant,
    snapshot: ModelsDevReasoningCatalogSnapshot,
}

#[cfg(feature = "product-full")]
fn parsed_catalog_cache() -> &'static RwLock<Option<CachedReasoningCatalogSnapshot>> {
    static CACHE: OnceLock<RwLock<Option<CachedReasoningCatalogSnapshot>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(None))
}

#[cfg(feature = "product-full")]
fn models_dev_catalog_service() -> &'static ModelsDevCatalogService {
    static SERVICE: OnceLock<ModelsDevCatalogService> = OnceLock::new();
    SERVICE.get_or_init(|| {
        let cache_file = crate::infrastructure::get_path_manager_arc()
            .cache_root()
            .join("models-dev")
            .join("catalog.json");
        ModelsDevCatalogService::new(cache_file)
    })
}

#[cfg(feature = "product-full")]
pub(crate) async fn load_models_dev_reasoning_catalog() -> ModelsDevReasoningCatalogSnapshot {
    if let Ok(cache) = parsed_catalog_cache().read() {
        if let Some(cached) = cache
            .as_ref()
            .filter(|cached| cached.loaded_at.elapsed() < CATALOG_RELOAD_INTERVAL)
        {
            return cached.snapshot.clone();
        }
    }

    let service = models_dev_catalog_service();
    let snapshot = service.load_cached_or_bundled().await;
    let catalog = match ModelsDevCatalog::parse_str(&snapshot.body) {
        Ok(catalog) => Some(Arc::new(catalog)),
        Err(error) => {
            debug!("Failed to parse models.dev catalog snapshot: {}", error);
            None
        }
    };

    let loaded = ModelsDevReasoningCatalogSnapshot {
        catalog,
        #[cfg(feature = "product-full")]
        version: snapshot.version,
    };
    if let Ok(mut cache) = parsed_catalog_cache().write() {
        *cache = Some(CachedReasoningCatalogSnapshot {
            loaded_at: Instant::now(),
            snapshot: loaded.clone(),
        });
    }

    let refresh_service = service.clone();
    tokio::spawn(async move {
        let ModelsDevRefreshOutcome::Updated(snapshot) = refresh_service.refresh_if_stale().await
        else {
            return;
        };
        let Some(updated) = parse_models_dev_snapshot(&snapshot) else {
            return;
        };
        if !replace_parsed_catalog_cache(updated) {
            return;
        }
        emit_models_dev_catalog_updated(&snapshot).await;
    });
    loaded
}

#[cfg(feature = "product-full")]
fn parse_models_dev_snapshot(
    snapshot: &ModelsDevSnapshot,
) -> Option<ModelsDevReasoningCatalogSnapshot> {
    let catalog = match ModelsDevCatalog::parse_str(&snapshot.body) {
        Ok(catalog) => Some(Arc::new(catalog)),
        Err(error) => {
            debug!(
                "Failed to parse refreshed models.dev catalog snapshot: {}",
                error
            );
            return None;
        }
    };
    Some(ModelsDevReasoningCatalogSnapshot {
        catalog,
        version: snapshot.version,
    })
}

#[cfg(feature = "product-full")]
fn replace_parsed_catalog_cache(updated: ModelsDevReasoningCatalogSnapshot) -> bool {
    let Ok(mut cache) = parsed_catalog_cache().write() else {
        return false;
    };
    replace_cached_catalog(&mut cache, updated)
}

#[cfg(feature = "product-full")]
fn replace_cached_catalog(
    cache: &mut Option<CachedReasoningCatalogSnapshot>,
    updated: ModelsDevReasoningCatalogSnapshot,
) -> bool {
    if cache
        .as_ref()
        .is_some_and(|cached| cached.snapshot.version == updated.version)
    {
        return false;
    }
    *cache = Some(CachedReasoningCatalogSnapshot {
        loaded_at: Instant::now(),
        snapshot: updated,
    });
    true
}

#[cfg(feature = "product-full")]
async fn emit_models_dev_catalog_updated(snapshot: &ModelsDevSnapshot) {
    let payload = match serde_json::to_value(AIModelCatalogUpdatedEvent {
        source_version: snapshot.version.to_string(),
        sha256: snapshot.sha256.clone(),
    }) {
        Ok(payload) => payload,
        Err(error) => {
            debug!(
                "Failed to serialize models.dev catalog update event: {}",
                error
            );
            return;
        }
    };
    let _ = crate::infrastructure::events::get_global_event_system()
        .emit(crate::infrastructure::events::BackendEvent::Custom {
            event_name: AI_MODEL_CATALOG_UPDATED_EVENT.to_string(),
            payload,
        })
        .await;
}

#[cfg(not(feature = "product-full"))]
pub(crate) async fn load_models_dev_reasoning_catalog() -> ModelsDevReasoningCatalogSnapshot {
    ModelsDevReasoningCatalogSnapshot { catalog: None }
}

pub(crate) fn project_model_reasoning_catalog(
    model: &AIModelConfig,
    models_dev: Option<&ModelsDevCatalog>,
) -> ReasoningCatalogProjection {
    project_reasoning_catalog(
        &model.provider,
        &model.model_name,
        &model.base_url,
        model.reasoning.as_ref(),
        models_dev,
    )
}

pub(crate) fn resolve_reasoning_preset<'a>(
    projection: &'a ReasoningCatalogProjection,
    preset_id: &str,
) -> Option<&'a ReasoningPresetDescriptor> {
    let preset_id = preset_id.trim();
    projection
        .presets
        .iter()
        .find(|preset| preset.id == preset_id)
}

pub(crate) fn resolve_default_reasoning_setting(
    projection: &ReasoningCatalogProjection,
) -> Option<&ReasoningPresetSetting> {
    projection
        .default_preset
        .as_deref()
        .and_then(|preset_id| resolve_reasoning_preset(projection, preset_id))
        .map(|preset| &preset.setting)
}

pub(crate) fn apply_default_reasoning_preset(
    client: AIClient,
    projection: &ReasoningCatalogProjection,
) -> AIClient {
    match resolve_default_reasoning_setting(projection) {
        Some(setting) => client.with_model_reasoning_preset(setting),
        None => client,
    }
}

pub(crate) fn apply_selected_reasoning_preset(
    client: &AIClient,
    projection: &ReasoningCatalogProjection,
    preset_id: &str,
) -> Option<AIClient> {
    resolve_reasoning_preset(projection, preset_id)
        .map(|preset| client.with_reasoning_preset(&preset.setting))
}

#[cfg(test)]
mod tests {
    use bitfun_core_types::{
        ReasoningCatalogBinding, ReasoningConfig, ReasoningMode, ReasoningPreset,
        ReasoningPresetSetting, ReasoningPresetSource,
    };

    use super::{
        apply_default_reasoning_preset, apply_selected_reasoning_preset,
        project_model_reasoning_catalog, resolve_default_reasoning_setting,
        resolve_reasoning_preset, ModelsDevCatalog,
    };
    use crate::infrastructure::ai::AIClient;
    use crate::service::config::types::AIModelConfig;
    use crate::util::types::AIConfig;

    fn catalog() -> ModelsDevCatalog {
        ModelsDevCatalog::parse_str(
            r#"{
                "openai": {"models": {
                    "gpt-test": {"id":"gpt-test","reasoning":true,
                        "reasoning_options":{"type":"effort","values":["low","high"]}}
                }}
            }"#,
        )
        .expect("models.dev fixture")
    }

    fn model(reasoning: Option<ReasoningConfig>) -> AIModelConfig {
        AIModelConfig {
            id: "model-1".to_string(),
            name: "GPT Test".to_string(),
            provider: "responses".to_string(),
            model_name: "gpt-test".to_string(),
            base_url: "https://api.openai.com/v1/responses".to_string(),
            reasoning,
            ..Default::default()
        }
    }

    fn runtime_config() -> AIConfig {
        AIConfig {
            name: "GPT Test".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            request_url: "https://api.openai.com/v1/responses".to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-test".to_string(),
            format: "responses".to_string(),
            context_window: 128_000,
            max_tokens: Some(8192),
            temperature: None,
            top_p: None,
            reasoning_mode: ReasoningMode::Default,
            inline_think_in_text: false,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            reasoning_effort: None,
            thinking_budget_tokens: None,
            custom_request_body: None,
            custom_request_body_mode: None,
        }
    }

    #[test]
    fn generated_preset_and_default_resolve_to_runtime_settings() {
        let projection = project_model_reasoning_catalog(
            &model(Some(ReasoningConfig {
                catalog: ReasoningCatalogBinding::Auto,
                default_preset: Some("high".to_string()),
                presets: Vec::new(),
            })),
            Some(&catalog()),
        );

        let high = resolve_reasoning_preset(&projection, "high").expect("generated high");
        assert_eq!(high.source, ReasoningPresetSource::ModelsDev);
        assert!(matches!(
            high.setting,
            ReasoningPresetSetting::Effort { ref value, .. } if value == "high"
        ));
        assert_eq!(
            resolve_default_reasoning_setting(&projection),
            Some(&high.setting)
        );
    }

    #[test]
    fn configured_override_and_disable_rules_match_the_projected_catalog() {
        let overridden = project_model_reasoning_catalog(
            &model(Some(ReasoningConfig {
                catalog: ReasoningCatalogBinding::Auto,
                default_preset: Some("high".to_string()),
                presets: vec![ReasoningPreset {
                    id: "high".to_string(),
                    setting: Some(ReasoningPresetSetting::Toggle { enabled: false }),
                    ..Default::default()
                }],
            })),
            Some(&catalog()),
        );
        let high = resolve_reasoning_preset(&overridden, "high").expect("configured override");
        assert_eq!(high.source, ReasoningPresetSource::ModelConfig);
        assert_eq!(
            high.setting,
            ReasoningPresetSetting::Toggle { enabled: false }
        );

        let disabled = project_model_reasoning_catalog(
            &model(Some(ReasoningConfig {
                catalog: ReasoningCatalogBinding::Auto,
                default_preset: Some("high".to_string()),
                presets: vec![ReasoningPreset {
                    id: "high".to_string(),
                    disabled: true,
                    ..Default::default()
                }],
            })),
            Some(&catalog()),
        );
        assert!(resolve_reasoning_preset(&disabled, "high").is_none());
        assert!(resolve_default_reasoning_setting(&disabled).is_none());
    }

    #[test]
    fn generated_default_and_session_presets_are_applied_to_runtime_clients() {
        let projection = project_model_reasoning_catalog(
            &model(Some(ReasoningConfig {
                catalog: ReasoningCatalogBinding::Auto,
                default_preset: Some("high".to_string()),
                presets: Vec::new(),
            })),
            Some(&catalog()),
        );
        let base = apply_default_reasoning_preset(AIClient::new(runtime_config()), &projection);
        assert_eq!(base.config.reasoning_effort.as_deref(), Some("high"));

        let selected = apply_selected_reasoning_preset(&base, &projection, "low")
            .expect("generated low session preset");
        assert_eq!(selected.config.reasoning_effort.as_deref(), Some("low"));
    }

    #[cfg(feature = "product-full")]
    #[test]
    fn refreshed_catalog_replaces_projection_without_waiting_for_reload_interval() {
        let mut cache = Some(super::CachedReasoningCatalogSnapshot {
            loaded_at: std::time::Instant::now(),
            snapshot: super::ModelsDevReasoningCatalogSnapshot {
                catalog: None,
                version: 1,
            },
        });
        let updated = super::ModelsDevReasoningCatalogSnapshot {
            catalog: None,
            version: 2,
        };

        assert!(super::replace_cached_catalog(&mut cache, updated));
        assert_eq!(cache.as_ref().map(|value| value.snapshot.version), Some(2));
    }
}
