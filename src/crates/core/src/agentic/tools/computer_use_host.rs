//! Host abstraction for desktop automation (implemented in `bitfun-desktop`).

use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Center of a **point crop** in **full-display native capture pixels** (same origin as ruler indices on a full-screen computer-use shot).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScreenshotCropCenter {
    pub x: u32,
    pub y: u32,
}

/// Native-pixel rectangle on the **captured display bitmap** (0..`native_width`, 0..`native_height`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComputerUseNavigationRect {
    pub x0: u32,
    pub y0: u32,
    pub width: u32,
    pub height: u32,
}

/// Subdivide the current navigation view into four tiles (model picks one per `screenshot` step).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputerUseNavigateQuadrant {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Parameters for [`ComputerUseHost::screenshot_display`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComputerUseScreenshotParams {
    pub crop_center: Option<ScreenshotCropCenter>,
    pub navigate_quadrant: Option<ComputerUseNavigateQuadrant>,
    /// Clear stored navigation focus before applying this capture (next quadrant step starts from full display).
    pub reset_navigation: bool,
    /// Half-size of the point crop in **native** pixels (total width/height ≈ `2 * half`). `None` → [`COMPUTER_USE_POINT_CROP_HALF_DEFAULT`].
    pub point_crop_half_extent_native: Option<u32>,
}

/// Longest side of the navigation region must be **strictly below** this to allow `click` without a separate point crop (desktop).
pub const COMPUTER_USE_QUADRANT_CLICK_READY_MAX_LONG_EDGE: u32 = 500;

/// Native pixels added on **each** side after a quadrant choice before compositing the JPEG (avoids controls sitting exactly on the split line).
pub const COMPUTER_USE_QUADRANT_EDGE_EXPAND_PX: u32 = 50;

/// Default **half** extent (native px) for point crop around `screenshot_crop_center_*` → total region up to **500×500**.
pub const COMPUTER_USE_POINT_CROP_HALF_DEFAULT: u32 = 250;

/// Minimum **half** extent for point crop (native px) — total region **≥ 128×128** when the display is large enough.
pub const COMPUTER_USE_POINT_CROP_HALF_MIN: u32 = 64;

/// Maximum **half** extent for point crop (native px) — total region **≤ 500×500**.
pub const COMPUTER_USE_POINT_CROP_HALF_MAX: u32 = 250;

/// Clamp optional model/host request to a valid point-crop half extent.
#[inline]
pub fn clamp_point_crop_half_extent(requested: Option<u32>) -> u32 {
    let v = requested.unwrap_or(COMPUTER_USE_POINT_CROP_HALF_DEFAULT);
    v.clamp(COMPUTER_USE_POINT_CROP_HALF_MIN, COMPUTER_USE_POINT_CROP_HALF_MAX)
}

/// Suggest a tighter half-extent from AX **native** bounds size (smaller controls → smaller JPEG).
#[inline]
pub fn suggested_point_crop_half_extent_from_native_bounds(native_w: u32, native_h: u32) -> u32 {
    let max_edge = native_w.max(native_h).max(1);
    let half = max_edge
        .saturating_div(2)
        .saturating_add(32);
    clamp_point_crop_half_extent(Some(half))
}

/// Snapshot of OS permissions relevant to computer use.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ComputerUsePermissionSnapshot {
    pub accessibility_granted: bool,
    pub screen_capture_granted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_note: Option<String>,
}

/// Frontmost application (for Computer use tool JSON).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ComputerUseForegroundApplication {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_id: Option<i32>,
}

/// Mouse cursor position in **global** screen space (host native units, e.g. macOS Quartz points).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComputerUsePointerGlobal {
    pub x: f64,
    pub y: f64,
}

/// Foreground app + pointer position after a Computer use action (best-effort per platform).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ComputerUseSessionSnapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreground_application: Option<ComputerUseForegroundApplication>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer_global: Option<ComputerUsePointerGlobal>,
}

/// Pixel rectangle of the **screen capture** inside the JPEG (excludes white margin and rulers).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComputerUseImageContentRect {
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
}

/// Screenshot payload for the model and for pointer coordinate mapping.
/// The `ComputerUse` tool embeds these fields in tool-result JSON and adds **`hierarchical_navigation`**
/// (`full_display` vs `region_crop`, plus **`shortcut_policy`**).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerScreenshot {
    pub bytes: Vec<u8>,
    pub mime_type: String,
    /// Dimensions of the image attached for the model (may be downscaled).
    pub image_width: u32,
    pub image_height: u32,
    /// Native capture dimensions for this display (before downscale).
    pub native_width: u32,
    pub native_height: u32,
    /// Top-left of this display in global screen space (for multi-monitor).
    pub display_origin_x: i32,
    pub display_origin_y: i32,
    /// Shrink factor for vision image vs native capture (Anthropic-style long-edge + megapixel cap).
    pub vision_scale: f64,
    /// When set, the **tip** of the drawn pointer overlay was placed at this pixel in the JPEG (`image_width` x `image_height`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer_image_x: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer_image_y: Option<i32>,
    /// When set, this JPEG is a crop around this center in **full-display native** pixels (see tool docs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot_crop_center: Option<ScreenshotCropCenter>,
    /// Half extent used for this point crop (native px); omitted when not a point crop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point_crop_half_extent_native: Option<u32>,
    /// Native rectangle corresponding to this JPEG’s content (full display, quadrant drill region, or point-crop bounds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub navigation_native_rect: Option<ComputerUseNavigationRect>,
    /// When true (desktop), `click` is allowed on this frame without an extra ~500×500 point crop — region is small enough for pointer positioning + `click`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub quadrant_navigation_click_ready: bool,
    /// Screen pixels inside the JPEG (below/left of white margin); `ComputerUseMousePrecise` maps this rect to the display.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_content_rect: Option<ComputerUseImageContentRect>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Filter for native accessibility (macOS AX) BFS search — role/title/identifier substrings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UiElementLocateQuery {
    #[serde(default)]
    pub title_contains: Option<String>,
    #[serde(default)]
    pub role_substring: Option<String>,
    #[serde(default)]
    pub identifier_contains: Option<String>,
    /// BFS depth from the application root (default 48, max 200).
    #[serde(default)]
    pub max_depth: Option<u32>,
    /// `"all"` (default): every non-empty filter must match the **same** element (AND).  
    /// `"any"`: at least one non-empty filter matches (OR) — useful when title and role are not both present on one node (e.g. search field with empty AXTitle).
    #[serde(default)]
    pub filter_combine: Option<String>,
}

/// Matched element geometry from the accessibility tree: center plus **axis-aligned bounds** (four corners).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiElementLocateResult {
    /// Same space as `ComputerUse` `use_screen_coordinates` / host pointer moves.
    pub global_center_x: f64,
    pub global_center_y: f64,
    /// Use with `ComputerUse` `screenshot_crop_center_x` / `y` (full-capture native indices).
    pub native_center_x: u32,
    pub native_center_y: u32,
    /// Element frame in **global** pointer space: top-left `(left, top)`, size `(width, height)`.
    /// Four corners: `(left, top)`, `(left+width, top)`, `(left, top+height)`, `(left+width, top+height)`.
    pub global_bounds_left: f64,
    pub global_bounds_top: f64,
    pub global_bounds_width: f64,
    pub global_bounds_height: f64,
    /// Tight **native** pixel bounds on the capture bitmap (full-display indices), derived from the global frame
    /// (mapping uses the display that contains the center; large spans may be approximate).
    pub native_bounds_min_x: u32,
    pub native_bounds_min_y: u32,
    pub native_bounds_max_x: u32,
    pub native_bounds_max_y: u32,
    pub matched_role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_identifier: Option<String>,
}

#[async_trait]
pub trait ComputerUseHost: Send + Sync + std::fmt::Debug {
    async fn permission_snapshot(&self) -> BitFunResult<ComputerUsePermissionSnapshot>;

    /// Platform-specific prompt (e.g. macOS accessibility dialog).
    async fn request_accessibility_permission(&self) -> BitFunResult<()>;

    /// Open settings or trigger OS screen-capture permission flow where supported.
    async fn request_screen_capture_permission(&self) -> BitFunResult<()>;

    /// Capture the display that contains `(0,0)`. See [`ComputerUseScreenshotParams`]: point crop, optional quadrant drill, refresh, reset.
    async fn screenshot_display(
        &self,
        params: ComputerUseScreenshotParams,
    ) -> BitFunResult<ComputerScreenshot>;

    /// Full-screen capture for **UI / human verification only**. Must **not** replace
    /// `last_pointer_map`, navigation focus, or `last_screenshot_refinement` (unlike [`screenshot_display`](Self::screenshot_display)).
    /// Desktop overrides with a side-effect-free capture; default delegates to a plain full-frame `screenshot_display` (may still advance navigation on naive embedders — override on desktop).
    async fn screenshot_peek_full_display(&self) -> BitFunResult<ComputerScreenshot> {
        self.screenshot_display(ComputerUseScreenshotParams::default())
            .await
    }

    /// Map `(x, y)` from the **last** screenshot's image pixel grid to global pointer pixels.
    /// Fails if no screenshot was taken in this process since startup (or since last host reset).
    fn map_image_coords_to_pointer(&self, x: i32, y: i32) -> BitFunResult<(i32, i32)>;

    /// Same as `map_image_coords_to_pointer` but **sub-point** precision (macOS: use for `ComputerUseMousePrecise`).
    fn map_image_coords_to_pointer_f64(&self, x: i32, y: i32) -> BitFunResult<(f64, f64)> {
        let (a, b) = self.map_image_coords_to_pointer(x, y)?;
        Ok((a as f64, b as f64))
    }

    /// Map `(x, y)` with each axis in `0..=1000` to the captured display in native pointer pixels.
    /// `(0,0)` ≈ top-left of capture, `(1000,1000)` ≈ bottom-right (inclusive mapping).
    fn map_normalized_coords_to_pointer(&self, x: i32, y: i32) -> BitFunResult<(i32, i32)>;

    fn map_normalized_coords_to_pointer_f64(&self, x: i32, y: i32) -> BitFunResult<(f64, f64)> {
        let (a, b) = self.map_normalized_coords_to_pointer(x, y)?;
        Ok((a as f64, b as f64))
    }

    /// Absolute move in host global display coordinates (on macOS: CG space, **double** precision).
    async fn mouse_move_global_f64(&self, gx: f64, gy: f64) -> BitFunResult<()> {
        self.mouse_move(gx.round() as i32, gy.round() as i32).await
    }

    async fn mouse_move(&self, x: i32, y: i32) -> BitFunResult<()>;

    /// Move the pointer by `(dx, dy)` in **global screen pixels** (same space as `ComputerUseMousePrecise` absolute).
    async fn pointer_move_relative(&self, dx: i32, dy: i32) -> BitFunResult<()>;

    /// Click at the **current** pointer position only (does not move). Use `ComputerUseMousePrecise` / `ComputerUseMouseStep` / `pointer_move_rel` first.
    /// `button`: "left" | "right" | "middle"
    async fn mouse_click(&self, button: &str) -> BitFunResult<()>;

    async fn scroll(&self, delta_x: i32, delta_y: i32) -> BitFunResult<()>;

    /// Press key combination; names like "command", "control", "shift", "alt", "return", "tab", "escape", "space", or single letters.
    async fn key_chord(&self, keys: Vec<String>) -> BitFunResult<()>;

    /// Type Unicode text (synthesized key events; may be imperfect for some IMEs).
    async fn type_text(&self, text: &str) -> BitFunResult<()>;

    async fn wait_ms(&self, ms: u64) -> BitFunResult<()>;

    /// Current frontmost app and global pointer position for tool-result JSON (`computer_use_context`).
    /// Default: empty. Desktop overrides with platform queries (typically after each tool action).
    async fn computer_use_session_snapshot(&self) -> ComputerUseSessionSnapshot {
        ComputerUseSessionSnapshot::default()
    }

    /// After a successful `screenshot_display`, the model may `mouse_click` (until the pointer moves again).
    fn computer_use_after_screenshot(&self) {}

    /// After `ComputerUseMousePrecise` / `ComputerUseMouseStep` / relative pointer moves: the next `mouse_click` must be preceded by a new screenshot.
    fn computer_use_after_pointer_mutation(&self) {}

    /// After `mouse_click`, require a fresh screenshot before the next click (unless pointer moved, which also invalidates).
    fn computer_use_after_click(&self) {}

    /// Refuse `mouse_click` if the pointer moved (or a click happened) since the last screenshot,
    /// or if the latest capture is not a valid “fine” basis (desktop: ~500×500 point crop **or**
    /// quadrant navigation region with longest side < [`COMPUTER_USE_QUADRANT_CLICK_READY_MAX_LONG_EDGE`]).
    fn computer_use_guard_click_allowed(&self) -> BitFunResult<()> {
        Ok(())
    }

    /// What the **last** `screenshot_display` captured (e.g. coordinate hints for the model).
    /// Default: unknown (`None`). Desktop sets after each `screenshot_display`.
    fn last_screenshot_refinement(&self) -> Option<ComputerUseScreenshotRefinement> {
        None
    }

    /// Search the frontmost app’s accessibility tree (macOS AX) for a matching control and return a stable center.
    /// Default: unsupported outside the desktop host / non-macOS.
    async fn locate_ui_element_screen_center(
        &self,
        _query: UiElementLocateQuery,
    ) -> BitFunResult<UiElementLocateResult> {
        Err(BitFunError::tool(
            "Native UI element (accessibility) lookup is not available on this host.".to_string(),
        ))
    }
}

/// Whether the latest screenshot JPEG was the full display, a point crop, or a quadrant-drill region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputerUseScreenshotRefinement {
    FullDisplay,
    RegionAroundPoint { center_x: u32, center_y: u32 },
    /// Partial-screen view from hierarchical quadrant navigation.
    QuadrantNavigation {
        x0: u32,
        y0: u32,
        width: u32,
        height: u32,
        click_ready: bool,
    },
}

pub type ComputerUseHostRef = std::sync::Arc<dyn ComputerUseHost>;
