//! macOS Accessibility (AX) tree search for stable UI centers (native “DOM”).
//!
//! Coordinates match CoreGraphics global space used by [`crate::computer_use::DesktopComputerUseHost`].

use crate::computer_use::ui_locate_common;
use bitfun_core::agentic::tools::computer_use_host::{UiElementLocateQuery, UiElementLocateResult};
use bitfun_core::util::errors::{BitFunError, BitFunResult};
use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::geometry::{CGPoint, CGSize};
use std::collections::VecDeque;
use std::ffi::c_void;

type AXUIElementRef = *const c_void;
type AXValueRef = *const c_void;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXValueGetType(value: AXValueRef) -> u32;
    fn AXValueGetValue(value: AXValueRef, the_type: u32, ptr: *mut c_void) -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRetain(cf: CFTypeRef) -> CFTypeRef;
}

const K_AX_VALUE_CGPOINT: u32 = 1;
const K_AX_VALUE_CGSIZE: u32 = 2;

fn frontmost_pid() -> BitFunResult<i32> {
    let out = std::process::Command::new("/usr/bin/osascript")
        .args([
            "-e",
            "tell application \"System Events\" to get unix id of first process whose frontmost is true",
        ])
        .output()
        .map_err(|e| BitFunError::tool(format!("osascript spawn: {}", e)))?;
    if !out.status.success() {
        return Err(BitFunError::tool(format!(
            "osascript failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    let s = String::from_utf8_lossy(&out.stdout);
    s.trim()
        .parse::<i32>()
        .map_err(|_| BitFunError::tool("Could not parse frontmost process id.".to_string()))
}

unsafe fn ax_release(v: CFTypeRef) {
    if !v.is_null() {
        core_foundation::base::CFRelease(v);
    }
}

unsafe fn ax_copy_attr(elem: AXUIElementRef, key: &str) -> Option<CFTypeRef> {
    let mut val: CFTypeRef = std::ptr::null();
    let k = CFString::new(key);
    let st = AXUIElementCopyAttributeValue(elem, k.as_concrete_TypeRef(), &mut val);
    if st != 0 || val.is_null() {
        if !val.is_null() {
            ax_release(val);
        }
        return None;
    }
    Some(val)
}

unsafe fn cfstring_to_string(cf: CFTypeRef) -> Option<String> {
    if cf.is_null() {
        return None;
    }
    let s = CFString::wrap_under_get_rule(cf as CFStringRef);
    Some(s.to_string())
}

unsafe fn ax_value_to_point(v: CFTypeRef) -> Option<CGPoint> {
    let v = v as AXValueRef;
    let t = AXValueGetType(v);
    if t != K_AX_VALUE_CGPOINT {
        return None;
    }
    let mut pt = CGPoint { x: 0.0, y: 0.0 };
    if !AXValueGetValue(v, K_AX_VALUE_CGPOINT, &mut pt as *mut _ as *mut c_void) {
        return None;
    }
    Some(pt)
}

unsafe fn ax_value_to_size(v: CFTypeRef) -> Option<CGSize> {
    let v = v as AXValueRef;
    let t = AXValueGetType(v);
    if t != K_AX_VALUE_CGSIZE {
        return None;
    }
    let mut sz = CGSize {
        width: 0.0,
        height: 0.0,
    };
    if !AXValueGetValue(v, K_AX_VALUE_CGSIZE, &mut sz as *mut _ as *mut c_void) {
        return None;
    }
    Some(sz)
}

unsafe fn read_role_title_id(elem: AXUIElementRef) -> (Option<String>, Option<String>, Option<String>) {
    let role = ax_copy_attr(elem, "AXRole").and_then(|v| {
        let s = cfstring_to_string(v);
        ax_release(v);
        s
    });
    let title = ax_copy_attr(elem, "AXTitle").and_then(|v| {
        let s = cfstring_to_string(v);
        ax_release(v);
        s
    });
    let ident = ax_copy_attr(elem, "AXIdentifier").and_then(|v| {
        let s = cfstring_to_string(v);
        ax_release(v);
        s
    });
    (role, title, ident)
}

/// Global center and axis-aligned bounds from `AXPosition` + `AXSize`.
unsafe fn element_frame_global(elem: AXUIElementRef) -> Option<(f64, f64, f64, f64, f64, f64)> {
    let pos = ax_copy_attr(elem, "AXPosition")?;
    let size = ax_copy_attr(elem, "AXSize")?;
    let pt = ax_value_to_point(pos)?;
    let sz = ax_value_to_size(size)?;
    ax_release(pos);
    ax_release(size);
    if sz.width <= 0.0 || sz.height <= 0.0 {
        return None;
    }
    let left = pt.x;
    let top = pt.y;
    let w = sz.width;
    let h = sz.height;
    Some((left + w / 2.0, top + h / 2.0, left, top, w, h))
}

struct Queued {
    ax: AXUIElementRef,
    depth: u32,
}

/// Search the **frontmost** app’s accessibility tree (BFS) for the first element matching filters.
pub fn locate_ui_element_center(query: &UiElementLocateQuery) -> BitFunResult<UiElementLocateResult> {
    ui_locate_common::validate_query(query)?;
    let max_depth = query.max_depth.unwrap_or(48).clamp(1, 200);
    let pid = frontmost_pid()?;
    let root = unsafe { AXUIElementCreateApplication(pid) };
    if root.is_null() {
        return Err(BitFunError::tool("AXUIElementCreateApplication returned null.".to_string()));
    }
    let mut q = VecDeque::new();
    q.push_back(Queued { ax: root, depth: 0 });
    let mut visited = 0usize;
    let max_nodes = 12_000usize;

    loop {
        let Some(cur) = q.pop_front() else {
            return Err(BitFunError::tool(
                "No accessibility element matched in the **frontmost** app. Filters default to **AND** (`filter_combine` omitted = `all`): every non-empty field must match the **same** node — e.g. `title_contains` + `role_substring` together often fails when the control has a **role** but **empty or different AXTitle** (typical for search fields). Try: **`filter_combine`: `\"any\"`**, or **only** `role_substring` (e.g. `TextField`), or **only** `title_contains`; match UI language; ensure the chat app is focused. Or use **`action: screenshot`**. (If Accessibility were denied, you would see a different error.)"
                    .to_string(),
            ));
        };
        if cur.depth > max_depth {
            unsafe {
                ax_release(cur.ax as CFTypeRef);
            }
            continue;
        }
        visited += 1;
        if visited > max_nodes {
            unsafe {
                ax_release(cur.ax as CFTypeRef);
            }
            while let Some(c) = q.pop_front() {
                unsafe {
                    ax_release(c.ax as CFTypeRef);
                }
            }
            return Err(BitFunError::tool(
                "Accessibility search limit reached; narrow title/role/identifier filters."
                    .to_string(),
            ));
        }

        let (role_s, title_s, id_s) = unsafe { read_role_title_id(cur.ax) };
        let role_ref = role_s.as_deref();
        let title_ref = title_s.as_deref();
        let id_ref = id_s.as_deref();

        let matched = ui_locate_common::matches_filters(query, role_ref, title_ref, id_ref);
        if matched {
            if let Some((gx, gy, bl, bt, bw, bh)) = unsafe { element_frame_global(cur.ax) } {
                unsafe {
                    ax_release(cur.ax as CFTypeRef);
                }
                return ui_locate_common::ok_result(
                    gx,
                    gy,
                    bl,
                    bt,
                    bw,
                    bh,
                    role_s.unwrap_or_default(),
                    title_s,
                    id_s,
                );
            }
        }

        let children_ref = unsafe { ax_copy_attr(cur.ax, "AXChildren") };
        let next_depth = cur.depth + 1;
        unsafe {
            ax_release(cur.ax as CFTypeRef);
        }

        let Some(ch) = children_ref else {
            continue;
        };
        unsafe {
            let arr = CFArray::<*const c_void>::wrap_under_create_rule(ch as CFArrayRef);
            let n = arr.len();
            for i in 0..n {
                let Some(child_ref) = arr.get(i) else {
                    continue;
                };
                let child = *child_ref;
                if child.is_null() {
                    continue;
                }
                let retained = CFRetain(child as CFTypeRef) as AXUIElementRef;
                if !retained.is_null() {
                    q.push_back(Queued {
                        ax: retained,
                        depth: next_depth,
                    });
                }
            }
        }
    }
}
