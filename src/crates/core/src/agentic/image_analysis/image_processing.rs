//! Shared image processing utilities used by both API-side image analysis and tool-driven image analysis.

use super::types::ImageLimits;
use crate::service::config::get_global_config_service;
use crate::service::config::types::{AIConfig as ServiceAIConfig, AIModelConfig, ModelCapability};
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::types::Message;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::imageops::FilterType;
use image::ColorType;
use image::DynamicImage;
use image::ImageEncoder;
use image::ImageFormat;
use serde_json::json;
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Debug, Clone)]
pub struct ProcessedImage {
    pub data: Vec<u8>,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
}

pub fn resolve_vision_model_from_ai_config(
    ai_config: &ServiceAIConfig,
) -> BitFunResult<AIModelConfig> {
    let target_model_id = ai_config
        .default_models
        .image_understanding
        .as_ref()
        .filter(|id| !id.is_empty());

    if let Some(id) = target_model_id {
        return ai_config
            .models
            .iter()
            .find(|m| m.id == *id)
            .cloned()
            .ok_or_else(|| BitFunError::service(format!("Model not found: {}", id)));
    }

    ai_config
        .models
        .iter()
        .find(|m| {
            m.enabled
                && m.capabilities
                    .iter()
                    .any(|cap| matches!(cap, ModelCapability::ImageUnderstanding))
        })
        .cloned()
        .ok_or_else(|| {
            BitFunError::service(
                "No image understanding model found.\nPlease configure an image understanding model in settings"
                    .to_string(),
            )
        })
}

pub async fn resolve_vision_model_from_global_config() -> BitFunResult<AIModelConfig> {
    let config_service = get_global_config_service().await?;
    let ai_config: ServiceAIConfig = config_service
        .get_config(Some("ai"))
        .await
        .map_err(|e| BitFunError::service(format!("Failed to get AI config: {}", e)))?;

    resolve_vision_model_from_ai_config(&ai_config)
}

pub fn resolve_image_path(path: &str, workspace_path: Option<&Path>) -> BitFunResult<PathBuf> {
    let path_buf = PathBuf::from(path);

    if path_buf.is_absolute() {
        Ok(path_buf)
    } else if let Some(workspace) = workspace_path {
        Ok(workspace.join(path_buf))
    } else {
        Ok(path_buf)
    }
}

pub async fn load_image_from_path(
    path: &Path,
    _workspace_path: Option<&Path>,
) -> BitFunResult<Vec<u8>> {
    fs::read(path)
        .await
        .map_err(|e| BitFunError::io(format!("Failed to read image: {}", e)))
}

pub fn decode_data_url(data_url: &str) -> BitFunResult<(Vec<u8>, Option<String>)> {
    if !data_url.starts_with("data:") {
        return Err(BitFunError::validation("Invalid data URL format"));
    }

    let parts: Vec<&str> = data_url.splitn(2, ',').collect();
    if parts.len() != 2 {
        return Err(BitFunError::validation("Data URL format error"));
    }

    let header = parts[0];
    let mime_type = header
        .strip_prefix("data:")
        .and_then(|s| s.split(';').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);

    let base64_data = parts[1];
    let image_data = BASE64
        .decode(base64_data)
        .map_err(|e| BitFunError::parse(format!("Base64 decode failed: {}", e)))?;

    Ok((image_data, mime_type))
}

pub fn detect_mime_type_from_bytes(
    image_data: &[u8],
    fallback_mime: Option<&str>,
) -> BitFunResult<String> {
    if let Ok(format) = image::guess_format(image_data) {
        if let Some(mime) = image_format_to_mime(format) {
            return Ok(mime.to_string());
        }
    }

    if let Some(fallback) = fallback_mime {
        if fallback.starts_with("image/") {
            return Ok(fallback.to_string());
        }
    }

    Err(BitFunError::validation(
        "Unsupported or unrecognized image format",
    ))
}

pub fn optimize_image_for_provider(
    image_data: Vec<u8>,
    provider: &str,
    fallback_mime: Option<&str>,
) -> BitFunResult<ProcessedImage> {
    let limits = ImageLimits::for_provider(provider);

    let guessed_format = image::guess_format(&image_data).ok();
    let dynamic = image::load_from_memory(&image_data)
        .map_err(|e| BitFunError::validation(format!("Failed to decode image data: {}", e)))?;

    let (orig_width, orig_height) = (dynamic.width(), dynamic.height());
    let needs_resize = orig_width > limits.max_width || orig_height > limits.max_height;

    if !needs_resize && image_data.len() <= limits.max_size {
        let mime_type = detect_mime_type_from_bytes(&image_data, fallback_mime)?;
        return Ok(ProcessedImage {
            data: image_data,
            mime_type,
            width: orig_width,
            height: orig_height,
        });
    }

    let mut working = if needs_resize {
        dynamic.resize(limits.max_width, limits.max_height, FilterType::Triangle)
    } else {
        dynamic
    };

    let preferred_format = match guessed_format {
        Some(ImageFormat::Jpeg) => ImageFormat::Jpeg,
        _ => ImageFormat::Png,
    };

    let mut encoded = encode_dynamic_image(&working, preferred_format, 85)?;

    if encoded.0.len() > limits.max_size {
        for quality in [80u8, 65, 50, 35] {
            encoded = encode_dynamic_image(&working, ImageFormat::Jpeg, quality)?;
            if encoded.0.len() <= limits.max_size {
                break;
            }
        }
    }

    if encoded.0.len() > limits.max_size {
        for _ in 0..3 {
            let next_w = ((working.width() as f32) * 0.85).round().max(64.0) as u32;
            let next_h = ((working.height() as f32) * 0.85).round().max(64.0) as u32;
            if next_w == working.width() && next_h == working.height() {
                break;
            }

            working = working.resize(next_w, next_h, FilterType::Triangle);

            for quality in [70u8, 55, 40] {
                encoded = encode_dynamic_image(&working, ImageFormat::Jpeg, quality)?;
                if encoded.0.len() <= limits.max_size {
                    break;
                }
            }

            if encoded.0.len() <= limits.max_size {
                break;
            }
        }
    }

    Ok(ProcessedImage {
        data: encoded.0,
        mime_type: encoded.1,
        width: working.width(),
        height: working.height(),
    })
}

pub fn build_multimodal_message(
    prompt: &str,
    image_data: &[u8],
    mime_type: &str,
    provider: &str,
) -> BitFunResult<Vec<Message>> {
    let base64_data = BASE64.encode(image_data);
    let provider_lower = provider.to_lowercase();

    let message = if provider_lower.contains("anthropic") {
        Message {
            role: "user".to_string(),
            content: Some(serde_json::to_string(&json!([
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": mime_type,
                        "data": base64_data
                    }
                },
                {
                    "type": "text",
                    "text": prompt
                }
            ]))?),
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    } else {
        // Default to OpenAI-compatible payload shape for OpenAI and most OpenAI-compatible providers.
        Message {
            role: "user".to_string(),
            content: Some(serde_json::to_string(&json!([
                {
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:{};base64,{}", mime_type, base64_data)
                    }
                },
                {
                    "type": "text",
                    "text": prompt
                }
            ]))?),
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    };

    Ok(vec![message])
}

fn image_format_to_mime(format: ImageFormat) -> Option<&'static str> {
    match format {
        ImageFormat::Png => Some("image/png"),
        ImageFormat::Jpeg => Some("image/jpeg"),
        ImageFormat::Gif => Some("image/gif"),
        ImageFormat::WebP => Some("image/webp"),
        ImageFormat::Bmp => Some("image/bmp"),
        _ => None,
    }
}

fn encode_dynamic_image(
    image: &DynamicImage,
    format: ImageFormat,
    jpeg_quality: u8,
) -> BitFunResult<(Vec<u8>, String)> {
    let target_format = match format {
        ImageFormat::Jpeg => ImageFormat::Jpeg,
        _ => ImageFormat::Png,
    };

    let mut buffer = Vec::new();

    match target_format {
        ImageFormat::Png => {
            let rgba = image.to_rgba8();
            let encoder = PngEncoder::new(&mut buffer);
            encoder
                .write_image(
                    rgba.as_raw(),
                    image.width(),
                    image.height(),
                    ColorType::Rgba8.into(),
                )
                .map_err(|e| BitFunError::tool(format!("PNG encode failed: {}", e)))?;
        }
        ImageFormat::Jpeg => {
            let mut encoder = JpegEncoder::new_with_quality(&mut buffer, jpeg_quality);
            encoder
                .encode_image(image)
                .map_err(|e| BitFunError::tool(format!("JPEG encode failed: {}", e)))?;
        }
        _ => unreachable!("unsupported target format"),
    }

    let mime = image_format_to_mime(target_format)
        .unwrap_or("image/png")
        .to_string();

    Ok((buffer, mime))
}
