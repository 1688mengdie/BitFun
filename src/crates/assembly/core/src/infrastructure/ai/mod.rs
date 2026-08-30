//! AI infrastructure
//!
//! Provides AI clients and related services

pub mod client_factory;
pub(crate) mod provider_catalog;
pub(crate) mod reasoning_catalog;
pub mod tool_call_accumulator;

use std::time::Duration;

pub use bitfun_ai_adapters::providers;
pub use bitfun_ai_adapters::stream as ai_stream_handlers;

pub use bitfun_ai_adapters::{AIClient, StreamOptions, StreamResponse};
#[cfg(feature = "subscription-auth")]
pub use client_factory::force_refresh_subscription_for_model;
pub use client_factory::{
    get_global_ai_client_factory, initialize_global_ai_client_factory, AIClientFactory,
};

use crate::service::config::types::{AIConfig, AIModelConfig};

pub fn build_stream_options(config: &AIConfig) -> StreamOptions {
    build_stream_options_for_model(config, None)
}

pub fn build_stream_options_for_model(
    config: &AIConfig,
    _model_config: Option<&AIModelConfig>,
) -> StreamOptions {
    let idle_timeout = config.stream_idle_timeout_secs.map(Duration::from_secs);
    let retry = &config.thresholds.model_retry;

    StreamOptions {
        idle_timeout,
        ttft_timeout: config.stream_ttft_timeout_secs.map(Duration::from_secs),
        max_attempts: retry.max_attempts,
        retry_base_delay_ms: retry.base_delay_ms,
        rate_limit_retry_base_delay_ms: retry.rate_limit_base_delay_ms,
        max_exponential_delay_ms: retry.max_exponential_delay_ms,
        max_rate_limit_delay_ms: retry.max_rate_limit_delay_ms,
        max_exponent_shift: retry.max_exponent_shift,
        connect_timeout_secs: config.stream_connect_timeout_secs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::config::types::{AIModelConfig, AiThresholdsConfig, ModelRetryThresholds};

    #[test]
    fn model_reasoning_config_does_not_override_stream_timeouts() {
        let config = AIConfig::default();
        let model = AIModelConfig {
            reasoning: Some(bitfun_core_types::ReasoningConfig::default()),
            ..Default::default()
        };

        let options = build_stream_options_for_model(&config, Some(&model));

        assert_eq!(options.ttft_timeout, Some(Duration::from_secs(600)));
        assert_eq!(options.idle_timeout, Some(Duration::from_secs(600)));
    }

    #[test]
    fn explicit_none_stream_timeouts_mean_wait_indefinitely() {
        let config = AIConfig {
            stream_idle_timeout_secs: None,
            stream_ttft_timeout_secs: None,
            ..Default::default()
        };

        let options = build_stream_options_for_model(&config, None);

        assert_eq!(options.ttft_timeout, None);
        assert_eq!(options.idle_timeout, None);
    }

    #[test]
    fn stream_options_carry_configured_retry_and_connect_timeout() {
        let config = AIConfig {
            stream_connect_timeout_secs: Some(42),
            thresholds: AiThresholdsConfig {
                model_retry: ModelRetryThresholds {
                    max_attempts: 3,
                    base_delay_ms: 100,
                    rate_limit_base_delay_ms: 200,
                    max_exponential_delay_ms: 1_000,
                    max_rate_limit_delay_ms: 2_000,
                    max_exponent_shift: 4,
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let options = build_stream_options_for_model(&config, None);

        assert_eq!(options.max_attempts, 3);
        assert_eq!(options.retry_base_delay_ms, 100);
        assert_eq!(options.rate_limit_retry_base_delay_ms, 200);
        assert_eq!(options.max_exponential_delay_ms, 1_000);
        assert_eq!(options.max_rate_limit_delay_ms, 2_000);
        assert_eq!(options.max_exponent_shift, 4);
        assert_eq!(options.connect_timeout_secs, Some(42));
    }

    #[test]
    fn stream_options_forward_none_connect_timeout_as_no_timeout() {
        let config = AIConfig {
            stream_connect_timeout_secs: None,
            ..Default::default()
        };

        let options = build_stream_options_for_model(&config, None);

        assert_eq!(options.connect_timeout_secs, None);
    }
}
