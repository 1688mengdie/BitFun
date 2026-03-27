mod builder;
mod payload;
mod render;
mod sanitize;
mod types;

use builder::build_entries_from_turns;
use payload::trim_payload_to_budget;
use render::render_payload_for_model;

pub use types::{CompressionFallbackOptions, CompressionReminder};

pub fn build_structured_compression_reminder(
    turns: Vec<Vec<crate::agentic::core::Message>>,
    options: &CompressionFallbackOptions,
) -> CompressionReminder {
    let entries = build_entries_from_turns(turns, options);
    let trimmed_payload = trim_payload_to_budget(entries, options);
    let model_text = render_payload_for_model(&trimmed_payload);

    CompressionReminder {
        model_text,
        payload: trimmed_payload,
        used_model_summary: false,
    }
}

#[cfg(test)]
mod tests;
