//! Accessibility tree locate — invoked as `ComputerUse` **`action: "locate"`** (same tool as screenshot / keys).

use crate::agentic::tools::computer_use_capability::computer_use_desktop_available;
use crate::agentic::tools::computer_use_host::{
    suggested_point_crop_half_extent_from_native_bounds, UiElementLocateQuery,
};
use crate::agentic::tools::implementations::computer_use_tool::computer_use_augment_result_json;
use crate::agentic::tools::framework::{ToolResult, ToolUseContext};
use crate::service::config::global::GlobalConfigManager;
use crate::util::errors::{BitFunError, BitFunResult};
use serde_json::{json, Value};

/// Runs native UI locate (AX / UIA / AT-SPI) for the foreground app — **`ComputerUse`** `action: "locate"`.
pub(crate) async fn execute_computer_use_locate(
    input: &Value,
    context: &ToolUseContext,
) -> BitFunResult<Vec<ToolResult>> {
    if context.agent_type.as_deref() != Some("Claw") {
        return Err(BitFunError::tool(
            "ComputerUse action locate is only available in Claw assistant mode.".to_string(),
        ));
    }
    if context.is_remote() {
        return Err(BitFunError::tool(
            "ComputerUse action locate cannot run while the session workspace is remote (SSH)."
                .to_string(),
        ));
    }
    if !computer_use_desktop_available() {
        return Err(BitFunError::tool(
            "Computer use is not available on this host.".to_string(),
        ));
    }
    let Ok(service) = GlobalConfigManager::get_service().await else {
        return Err(BitFunError::tool(
            "Computer use configuration is unavailable.".to_string(),
        ));
    };
    let ai: crate::service::config::types::AIConfig =
        service.get_config(Some("ai")).await.unwrap_or_default();
    if !ai.computer_use_enabled {
        return Err(BitFunError::tool(
            "Computer use is disabled in BitFun settings.".to_string(),
        ));
    }

    let host = context.computer_use_host.as_ref().ok_or_else(|| {
        BitFunError::tool("Computer use is only available in the BitFun desktop app.".to_string())
    })?;

    let query = UiElementLocateQuery {
        title_contains: input
            .get("title_contains")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        role_substring: input
            .get("role_substring")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        identifier_contains: input
            .get("identifier_contains")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        max_depth: input
            .get("max_depth")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32),
        filter_combine: input
            .get("filter_combine")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    };

    let input_coords = json!({
        "kind": "locate",
        "title_contains": query.title_contains.clone(),
        "role_substring": query.role_substring.clone(),
        "identifier_contains": query.identifier_contains.clone(),
        "max_depth": query.max_depth,
        "filter_combine": query.filter_combine.clone(),
    });

    let res = host.locate_ui_element_screen_center(query).await?;

    let native_w = res
        .native_bounds_max_x
        .saturating_sub(res.native_bounds_min_x)
        .saturating_add(1);
    let native_h = res
        .native_bounds_max_y
        .saturating_sub(res.native_bounds_min_y)
        .saturating_add(1);

    let gx = res.global_center_x.round() as i64;
    let gy = res.global_center_y.round() as i64;
    let ncx = res.native_center_x as i64;
    let ncy = res.native_center_y as i64;

    let suggested_half = suggested_point_crop_half_extent_from_native_bounds(native_w, native_h);

    let coordinate_hints = json!({
        "mouse_precise_screen": {
            "tool": "ComputerUseMousePrecise",
            "use_screen_coordinates": true,
            "x": gx,
            "y": gy,
            "note": "Global display coordinates (host native units, e.g. macOS points). No prior screenshot required."
        },
        "mouse_precise_image_after_full_screenshot": {
            "tool": "ComputerUseMousePrecise",
            "use_screen_coordinates": false,
            "coordinate_mode": "image",
            "x": ncx,
            "y": ncy,
            "note": "Use only when the last ComputerUse screenshot was full-display; x/y match margin ruler indices on that JPEG. After a point-crop screenshot, image space is the crop — do not reuse these numbers."
        },
        "screenshot_point_crop": {
            "tool": "ComputerUse",
            "action": "screenshot",
            "screenshot_crop_center_x": res.native_center_x,
            "screenshot_crop_center_y": res.native_center_y,
            "screenshot_crop_half_extent_native": suggested_half,
            "note": "Copy **`screenshot_crop_center_*`** and **`screenshot_crop_half_extent_native`** into **`ComputerUse`** `action: \"screenshot\"`. Half-extent is derived from `native_extent_*` (tighter on small controls; host clamps)."
        },
        "native_extent_px": {
            "width": native_w,
            "height": native_h,
            "note": "Approximate control size in full-display native pixels; prefer smaller ComputerUseMouseStep pixels when width/height are small."
        }
    });

    let body = json!({
        "success": true,
        "action": "locate",
        "global_center_x": res.global_center_x,
        "global_center_y": res.global_center_y,
        "native_center_x": res.native_center_x,
        "native_center_y": res.native_center_y,
        "global_bounds_left": res.global_bounds_left,
        "global_bounds_top": res.global_bounds_top,
        "global_bounds_width": res.global_bounds_width,
        "global_bounds_height": res.global_bounds_height,
        "native_bounds_min_x": res.native_bounds_min_x,
        "native_bounds_min_y": res.native_bounds_min_y,
        "native_bounds_max_x": res.native_bounds_max_x,
        "native_bounds_max_y": res.native_bounds_max_y,
        "native_extent_width": native_w,
        "native_extent_height": native_h,
        "coordinate_hints": coordinate_hints,
        "matched_role": res.matched_role,
        "matched_title": res.matched_title,
        "matched_identifier": res.matched_identifier,
        "recommended_next": "Prefer **`ComputerUse`** `action: screenshot` with fields from `coordinate_hints.screenshot_point_crop` to narrow the JPEG before quadrant drill; then ComputerUseMousePrecise / ComputerUseMouseStep + ComputerUseMouseClick, or use mouse_precise_screen if no screenshot is needed yet."
    });

    let body = computer_use_augment_result_json(host.as_ref(), body, Some(input_coords)).await;

    let summary = format!(
        "AX match: role={} native_center=({}, {}) native_bounds=[{}..{}, {}..{}] global_center=({:.1}, {:.1})",
        res.matched_role,
        res.native_center_x,
        res.native_center_y,
        res.native_bounds_min_x,
        res.native_bounds_max_x,
        res.native_bounds_min_y,
        res.native_bounds_max_y,
        res.global_center_x,
        res.global_center_y
    );

    Ok(vec![ToolResult::ok(body, Some(summary))])
}
