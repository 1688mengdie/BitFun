use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;
use tauri::AppHandle;
use tauri::Listener;
use tauri::Manager;
use tokio::sync::oneshot;

use crate::server::response::WebDriverErrorResponse;
use crate::server::AppState;
use crate::webdriver::FrameId;

const BRIDGE_EVENT: &str = "bitfun_webdriver_result";

#[derive(Debug, Deserialize)]
pub struct BridgeResponse {
    #[serde(rename = "requestId")]
    request_id: String,
    ok: bool,
    value: Option<Value>,
    error: Option<BridgeError>,
}

#[derive(Debug, Deserialize)]
struct BridgeError {
    message: Option<String>,
    stack: Option<String>,
}

pub fn register_listener(app: AppHandle, state: Arc<AppState>) {
    app.listen_any(BRIDGE_EVENT, move |event| {
        let Ok(payload) = serde_json::from_str::<BridgeResponse>(event.payload()) else {
            return;
        };

        let maybe_sender = state
            .pending_requests
            .lock()
            .ok()
            .and_then(|mut pending| pending.remove(&payload.request_id));

        if let Some(sender) = maybe_sender {
            let _ = sender.send(payload);
        }
    });
}

pub async fn run_script(
    state: Arc<AppState>,
    session_id: &str,
    script: &str,
    args: Vec<Value>,
    async_mode: bool,
) -> Result<Value, WebDriverErrorResponse> {
    let session = state.sessions.read().await.get_cloned(session_id)?;
    let request_id = state.next_request_id();
    let timeout_ms = session.timeouts.script.max(5_000);
    let (sender, receiver) = oneshot::channel();

    state
        .pending_requests
        .lock()
        .map_err(|_| WebDriverErrorResponse::unknown_error("Failed to lock pending request map"))?
        .insert(request_id.clone(), sender);

    let webview = state
        .app
        .get_webview(&session.current_window)
        .ok_or_else(|| {
            WebDriverErrorResponse::no_such_window(format!(
                "Webview not found: {}",
                session.current_window
            ))
        })?;

    let frame_context = serialize_frame_context(&session.frame_context);
    let injected = build_bridge_eval_script(&request_id, script, &args, async_mode, &frame_context);
    webview.eval(&injected).map_err(|error| {
        remove_pending_request(&state, &request_id);
        WebDriverErrorResponse::javascript_error(format!("Failed to evaluate script: {error}"), None)
    })?;

    let response = tokio::time::timeout(Duration::from_millis(timeout_ms), receiver)
        .await
        .map_err(|_| {
            remove_pending_request(&state, &request_id);
            WebDriverErrorResponse::timeout(format!("Script timed out after {timeout_ms}ms"))
        })?
        .map_err(|_| WebDriverErrorResponse::unknown_error("Bridge response channel closed unexpectedly"))?;

    if response.ok {
        return Ok(response.value.unwrap_or(Value::Null));
    }

    let error = response.error.unwrap_or(BridgeError {
        message: Some("Unknown JavaScript error".into()),
        stack: None,
    });
    Err(WebDriverErrorResponse::javascript_error(
        error
            .message
            .unwrap_or_else(|| "Unknown JavaScript error".into()),
        error.stack,
    ))
}

fn remove_pending_request(state: &AppState, request_id: &str) {
    if let Ok(mut pending) = state.pending_requests.lock() {
        pending.remove(request_id);
    }
}

fn serialize_frame_context(frame_context: &[FrameId]) -> Value {
    Value::Array(
        frame_context
            .iter()
            .map(|frame_id| match frame_id {
                FrameId::Index(index) => serde_json::json!({
                    "kind": "index",
                    "value": index
                }),
                FrameId::Element(element_id) => serde_json::json!({
                    "kind": "element",
                    "value": element_id
                }),
            })
            .collect(),
    )
}

fn build_bridge_eval_script(
    request_id: &str,
    script: &str,
    args: &[Value],
    async_mode: bool,
    frame_context: &Value,
) -> String {
    let request_id_json =
        serde_json::to_string(request_id).unwrap_or_else(|_| "\"invalid-request\"".into());
    let script_json = serde_json::to_string(script).unwrap_or_else(|_| "\"\"".into());
    let args_json = serde_json::to_string(args).unwrap_or_else(|_| "[]".into());
    let async_json = if async_mode { "true" } else { "false" };
    let frame_context_json =
        serde_json::to_string(frame_context).unwrap_or_else(|_| "[]".into());

    format!(
        r#"
(() => {{
  {helper}
  window.__bitfunWd.run({request_id}, {script}, {args}, {async_mode}, {frame_context});
}})();
"#,
        helper = bridge_helper_script(),
        request_id = request_id_json,
        script = script_json,
        args = args_json,
        async_mode = async_json,
        frame_context = frame_context_json
    )
}

fn bridge_helper_script() -> &'static str {
    r#"
if (!window.__bitfunWd) {
  window.__bitfunWd = (() => {
    const ELEMENT_KEY = "element-6066-11e4-a52e-4f735466cecf";
    const SHADOW_KEY = "shadow-6066-11e4-a52e-4f735466cecf";
    const EVENT_NAME = "bitfun_webdriver_result";
    const STORE_KEY = "__bitfunWdElements";
    const LOG_KEY = "__bitfunWdLogs";
    const consolePatchedKey = "__bitfunWdConsolePatched";
    let currentFrameContext = [];

    const ensureStore = () => {
      if (!window[STORE_KEY]) {
        window[STORE_KEY] = Object.create(null);
      }
      return window[STORE_KEY];
    };

    const ensureLogs = () => {
      if (!window[LOG_KEY]) {
        window[LOG_KEY] = [];
      }
      return window[LOG_KEY];
    };

    const safeStringify = (value) => {
      if (typeof value === "string") {
        return value;
      }
      try {
        return JSON.stringify(value);
      } catch (_error) {
        return String(value);
      }
    };

    const cssEscape = (value) => {
      if (typeof CSS !== "undefined" && typeof CSS.escape === "function") {
        return CSS.escape(String(value));
      }
      return String(value).replace(/[^a-zA-Z0-9_\u00A0-\uFFFF-]/g, (char) => `\\${char}`);
    };

    const setFrameContext = (frameContext) => {
      currentFrameContext = Array.isArray(frameContext) ? frameContext : [];
    };

    const getFrameContext = () => currentFrameContext;

    const patchConsole = () => {
      if (window[consolePatchedKey]) {
        return;
      }
      window[consolePatchedKey] = true;
      ["log", "info", "warn", "error", "debug"].forEach((level) => {
        const original = console[level];
        console[level] = (...args) => {
          try {
            ensureLogs().push({
              level: level === "warn" ? "WARNING" : level === "error" ? "SEVERE" : "INFO",
              message: args.map((item) => safeStringify(item)).join(" "),
              timestamp: Date.now()
            });
            if (ensureLogs().length > 200) {
              ensureLogs().splice(0, ensureLogs().length - 200);
            }
          } catch (_error) {}
          return original.apply(console, args);
        };
      });
    };

    const ensureAlertState = (targetWindow = window) => {
      if (!targetWindow.__bitfunWdAlertState) {
        targetWindow.__bitfunWdAlertState = {
          open: false,
          type: null,
          text: "",
          defaultValue: null,
          promptText: null
        };
      }
      return targetWindow.__bitfunWdAlertState;
    };

    const patchDialogs = (targetWindow = window) => {
      const patchedKey = "__bitfunWdDialogsPatched";
      if (targetWindow[patchedKey]) {
        return;
      }
      targetWindow[patchedKey] = true;

      const state = ensureAlertState(targetWindow);
      targetWindow.alert = (message) => {
        state.open = true;
        state.type = "alert";
        state.text = String(message ?? "");
        state.defaultValue = null;
        state.promptText = null;
      };
      targetWindow.confirm = (message) => {
        state.open = true;
        state.type = "confirm";
        state.text = String(message ?? "");
        state.defaultValue = null;
        state.promptText = null;
        return false;
      };
      targetWindow.prompt = (message, defaultValue = "") => {
        state.open = true;
        state.type = "prompt";
        state.text = String(message ?? "");
        state.defaultValue = defaultValue == null ? null : String(defaultValue);
        state.promptText = defaultValue == null ? null : String(defaultValue);
        return null;
      };
    };

    const emitResult = async (payload) => {
      if (!window.__TAURI__ || !window.__TAURI__.event || typeof window.__TAURI__.event.emit !== "function") {
        throw new Error("Tauri event bridge unavailable");
      }
      await window.__TAURI__.event.emit(EVENT_NAME, payload);
    };

    const nextElementId = () => {
      window.__bitfunWdElementCounter = (window.__bitfunWdElementCounter || 0) + 1;
      return `bf-el-${window.__bitfunWdElementCounter}`;
    };

    const storeElement = (element) => {
      if (!element || typeof element !== "object") {
        return null;
      }
      const store = ensureStore();
      const existing = Object.entries(store).find(([, candidate]) => candidate === element);
      const id = existing ? existing[0] : nextElementId();
      store[id] = element;
      return { [ELEMENT_KEY]: id, ELEMENT: id };
    };

    const storeShadowRoot = (shadowRoot) => {
      if (!shadowRoot || typeof shadowRoot !== "object") {
        return null;
      }
      const store = ensureStore();
      const existing = Object.entries(store).find(([, candidate]) => candidate === shadowRoot);
      const id = existing ? existing[0] : nextElementId();
      store[id] = shadowRoot;
      return { [SHADOW_KEY]: id };
    };

    const getElement = (elementId) => {
      if (!elementId) {
        return null;
      }
      return ensureStore()[elementId] || null;
    };

    const isElementLike = (value) => !!value && typeof value === "object" && value.nodeType === 1;

    const getCurrentWindow = (frameContext = currentFrameContext) => {
      let currentWindowRef = window;
      for (const frameRef of frameContext || []) {
        let frameElement = null;
        if (!frameRef || typeof frameRef !== "object") {
          throw new Error("Invalid frame reference");
        }
        if (frameRef.kind === "index") {
          const frames = Array.from(currentWindowRef.document.querySelectorAll("iframe, frame"));
          frameElement = frames[Number(frameRef.value)];
        } else if (frameRef.kind === "element") {
          frameElement = getElement(String(frameRef.value));
        } else {
          throw new Error("Unsupported frame reference");
        }

        if (!frameElement || !isElementLike(frameElement)) {
          throw new Error("Unable to locate frame");
        }
        if (!/^(iframe|frame)$/i.test(String(frameElement.tagName || ""))) {
          throw new Error("Element is not a frame");
        }
        if (!frameElement.contentWindow) {
          throw new Error("Frame window is not available");
        }
        currentWindowRef = frameElement.contentWindow;
      }
      return currentWindowRef;
    };

    const getCurrentDocument = (frameContext = currentFrameContext) => {
      const currentWindowRef = getCurrentWindow(frameContext);
      if (!currentWindowRef.document) {
        throw new Error("Frame document is not available");
      }
      return currentWindowRef.document;
    };

    const serialize = (value, seen = new WeakSet()) => {
      if (value === undefined || value === null) {
        return value ?? null;
      }
      if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
        return value;
      }
      if (isElementLike(value)) {
        return storeElement(value);
      }
      if (value && typeof value === "object" && typeof value.length === "number" && typeof value !== "string") {
        return Array.from(value).map((item) => serialize(item, seen));
      }
      if (value && typeof value === "object" && "x" in value && "y" in value && "width" in value && "height" in value && "top" in value && "left" in value) {
        return {
          x: value.x,
          y: value.y,
          width: value.width,
          height: value.height,
          top: value.top,
          right: value.right,
          bottom: value.bottom,
          left: value.left
        };
      }
      if (value && typeof value === "object" && "message" in value && "stack" in value) {
        return {
          name: value.name,
          message: value.message,
          stack: value.stack
        };
      }
      if (Array.isArray(value)) {
        return value.map((item) => serialize(item, seen));
      }
      if (typeof value === "object") {
        if (seen.has(value)) {
          return null;
        }
        seen.add(value);
        const out = {};
        Object.keys(value).forEach((key) => {
          out[key] = serialize(value[key], seen);
        });
        return out;
      }
      return String(value);
    };

    const deserialize = (value) => {
      if (Array.isArray(value)) {
        return value.map(deserialize);
      }
      if (value && typeof value === "object") {
        if (typeof value[ELEMENT_KEY] === "string") {
          return getElement(value[ELEMENT_KEY]);
        }
        if (typeof value[SHADOW_KEY] === "string") {
          return getElement(value[SHADOW_KEY]);
        }
        const out = {};
        Object.keys(value).forEach((key) => {
          out[key] = deserialize(value[key]);
        });
        return out;
      }
      return value;
    };

    const resolveRoot = (rootId, frameContext = currentFrameContext) => {
      if (!rootId) {
        return getCurrentDocument(frameContext);
      }
      return getElement(rootId) || getCurrentDocument(frameContext);
    };

    const findByXpath = (root, xpath) => {
      const results = [];
      const ownerDocument = root && root.ownerDocument ? root.ownerDocument : getCurrentDocument();
      const iterator = ownerDocument.evaluate(
        xpath,
        root,
        null,
        XPathResult.ORDERED_NODE_ITERATOR_TYPE,
        null
      );
      let node = iterator.iterateNext();
      while (node) {
        if (isElementLike(node)) {
          results.push(node);
        }
        node = iterator.iterateNext();
      }
      return results;
    };

    const findElements = (rootId, using, value, frameContext = currentFrameContext) => {
      const root = resolveRoot(rootId, frameContext);
      let matches = [];
      switch (using) {
        case "css selector":
          matches = Array.from(root.querySelectorAll(value));
          break;
        case "id":
          matches = Array.from(root.querySelectorAll(`#${cssEscape(value)}`));
          break;
        case "name":
          matches = Array.from(root.querySelectorAll(`[name="${cssEscape(value)}"]`));
          break;
        case "class name":
          matches = Array.from(root.getElementsByClassName(value));
          break;
        case "xpath":
          matches = findByXpath(root, value);
          break;
        case "link text":
          matches = Array.from(root.querySelectorAll("a")).filter((item) => (item.textContent || "").trim() === value);
          break;
        case "partial link text":
          matches = Array.from(root.querySelectorAll("a")).filter((item) => (item.textContent || "").includes(value));
          break;
        case "tag name":
          matches = Array.from(root.querySelectorAll(value));
          break;
        default:
          throw new Error(`Unsupported locator strategy: ${using}`);
      }
      return matches.map((item) => storeElement(item));
    };

    const validateFrameByIndex = (index, frameContext = currentFrameContext) => {
      const currentDocumentRef = getCurrentDocument(frameContext);
      const frames = Array.from(currentDocumentRef.querySelectorAll("iframe, frame"));
      return Number.isInteger(index) && index >= 0 && index < frames.length;
    };

    const validateFrameElement = (elementId) => {
      const element = getElement(elementId);
      return !!element && isElementLike(element) && /^(iframe|frame)$/i.test(String(element.tagName || "")) && !!element.contentWindow;
    };

    const getShadowRoot = (elementId) => {
      const element = getElement(elementId);
      if (!element || !isElementLike(element) || !element.shadowRoot) {
        return null;
      }
      return storeShadowRoot(element.shadowRoot);
    };

    const findElementsFromShadow = (shadowId, using, value, frameContext = currentFrameContext) => {
      const shadowRoot = getElement(shadowId);
      if (!shadowRoot) {
        throw new Error("No shadow root found");
      }
      return findElements(shadowId, using, value, frameContext);
    };

    const isDisplayed = (element) => {
      if (!element || !element.isConnected) {
        return false;
      }
      const style = window.getComputedStyle(element);
      if (style.display === "none" || style.visibility === "hidden" || style.visibility === "collapse") {
        return false;
      }
      if (Number(style.opacity || "1") === 0) {
        return false;
      }
      const rect = element.getBoundingClientRect();
      return rect.width > 0 && rect.height > 0;
    };

    const setSelectionRange = (element, start, end) => {
      if (typeof element.setSelectionRange === "function") {
        element.setSelectionRange(start, end);
      }
    };

    const emitInputEvents = (element) => {
      element.dispatchEvent(new Event("input", { bubbles: true }));
      element.dispatchEvent(new Event("change", { bubbles: true }));
    };

    const clearElement = (element) => {
      if (!element) {
        return;
      }
      if ("value" in element) {
        element.focus();
        element.value = "";
        emitInputEvents(element);
        return;
      }
      if (element.isContentEditable) {
        element.focus();
        element.textContent = "";
        emitInputEvents(element);
      }
    };

    const insertText = (element, text) => {
      if (!element) {
        return;
      }
      if ("value" in element) {
        const currentValue = String(element.value || "");
        const start = typeof element.selectionStart === "number" ? element.selectionStart : currentValue.length;
        const end = typeof element.selectionEnd === "number" ? element.selectionEnd : currentValue.length;
        const nextValue = currentValue.slice(0, start) + text + currentValue.slice(end);
        element.value = nextValue;
        const caret = start + text.length;
        setSelectionRange(element, caret, caret);
        emitInputEvents(element);
        return;
      }
      if (element.isContentEditable) {
        const selection = window.getSelection();
        element.focus();
        if (selection && selection.rangeCount > 0) {
          selection.deleteFromDocument();
          selection.getRangeAt(0).insertNode(document.createTextNode(text));
          selection.collapseToEnd();
        } else {
          element.appendChild(document.createTextNode(text));
        }
        emitInputEvents(element);
      }
    };

    const setElementText = (element, text) => {
      clearElement(element);
      insertText(element, text);
    };

    const dispatchKeyboardEvent = (target, type, key, modifiers) => {
      if (!target) {
        return true;
      }
      return target.dispatchEvent(
        new KeyboardEvent(type, {
          key,
          code: key,
          bubbles: true,
          cancelable: true,
          ctrlKey: modifiers.ctrl,
          shiftKey: modifiers.shift,
          altKey: modifiers.alt,
          metaKey: modifiers.meta
        })
      );
    };

    const applySpecialKey = (target, key, modifiers) => {
      if (!target) {
        return;
      }

      const isInputLike = "value" in target;
      if ((modifiers.ctrl || modifiers.meta) && key.toLowerCase() === "a" && isInputLike) {
        const value = String(target.value || "");
        setSelectionRange(target, 0, value.length);
        return;
      }

      if (key === "Backspace" && isInputLike) {
        const value = String(target.value || "");
        const start = typeof target.selectionStart === "number" ? target.selectionStart : value.length;
        const end = typeof target.selectionEnd === "number" ? target.selectionEnd : value.length;
        if (start !== end) {
          target.value = value.slice(0, start) + value.slice(end);
          setSelectionRange(target, start, start);
        } else if (start > 0) {
          target.value = value.slice(0, start - 1) + value.slice(end);
          setSelectionRange(target, start - 1, start - 1);
        }
        emitInputEvents(target);
        return;
      }

      if (key === "Enter") {
        if (isInputLike && target.tagName === "TEXTAREA" && !modifiers.ctrl && !modifiers.meta) {
          insertText(target, "\n");
        }
        return;
      }

      if (key.length === 1 && !modifiers.ctrl && !modifiers.meta && !modifiers.alt) {
        insertText(target, key);
      }
    };

    const dispatchPointerClick = (element, button, doubleClick) => {
      if (!element) {
        throw new Error("Element not found");
      }
      const rect = element.getBoundingClientRect();
      const options = {
        bubbles: true,
        cancelable: true,
        clientX: rect.left + rect.width / 2,
        clientY: rect.top + rect.height / 2,
        button,
        buttons: button === 2 ? 2 : 1
      };
      element.scrollIntoView({ block: "center", inline: "center" });
      if (typeof element.focus === "function") {
        element.focus();
      }
      element.dispatchEvent(new MouseEvent("mouseover", options));
      element.dispatchEvent(new MouseEvent("mousemove", options));
      element.dispatchEvent(new MouseEvent("mousedown", options));
      element.dispatchEvent(new MouseEvent("mouseup", options));
      if (button === 2) {
        element.dispatchEvent(new MouseEvent("contextmenu", options));
        return;
      }
      element.dispatchEvent(new MouseEvent("click", options));
      if (doubleClick) {
        element.dispatchEvent(new MouseEvent("dblclick", options));
      }
    };

    const performActions = (sources) => {
      let pointerTarget = null;
      const keyState = { ctrl: false, shift: false, alt: false, meta: false };
      sources.forEach((source) => {
        if (!source || !Array.isArray(source.actions)) {
          return;
        }

        if (source.type === "pointer") {
          let sawClick = false;
          let button = 0;
          source.actions.forEach((action) => {
            if (action.type === "pointerMove" && action.origin && typeof action.origin[ELEMENT_KEY] === "string") {
              pointerTarget = getElement(action.origin[ELEMENT_KEY]);
            } else if (action.type === "pointerDown") {
              button = Number(action.button || 0);
            } else if (action.type === "pointerUp") {
              if (pointerTarget) {
                dispatchPointerClick(pointerTarget, button, sawClick && button === 0);
              }
              sawClick = true;
            }
          });
          return;
        }

        if (source.type === "key") {
          source.actions.forEach((action) => {
            if (action.type === "pause") {
              return;
            }
            const target = document.activeElement || document.body;
            const key = String(action.value || "");
            if (action.type === "keyDown") {
              if (key === "Control") keyState.ctrl = true;
              if (key === "Shift") keyState.shift = true;
              if (key === "Alt") keyState.alt = true;
              if (key === "Meta") keyState.meta = true;
              dispatchKeyboardEvent(target, "keydown", key, keyState);
              applySpecialKey(target, key, keyState);
              if (key.length === 1) {
                dispatchKeyboardEvent(target, "keypress", key, keyState);
              }
              return;
            }
            dispatchKeyboardEvent(target, "keyup", key, keyState);
            if (key === "Control") keyState.ctrl = false;
            if (key === "Shift") keyState.shift = false;
            if (key === "Alt") keyState.alt = false;
            if (key === "Meta") keyState.meta = false;
          });
        }
      });
    };

    const takeLogs = () => {
      const logs = ensureLogs().slice();
      ensureLogs().length = 0;
      return logs;
    };

    const parseDocumentCookies = (doc) => {
      const raw = doc.cookie || "";
      if (!raw.trim()) {
        return [];
      }
      return raw
        .split(/;\s*/)
        .filter(Boolean)
        .map((entry) => {
          const separator = entry.indexOf("=");
          const name = separator >= 0 ? entry.slice(0, separator) : entry;
          const value = separator >= 0 ? entry.slice(separator + 1) : "";
          return {
            name: decodeURIComponent(name),
            value: decodeURIComponent(value),
            path: null,
            domain: null,
            secure: false,
            httpOnly: false,
            expiry: null,
            sameSite: null
          };
        });
    };

    const getAllCookies = (frameContext = currentFrameContext) => parseDocumentCookies(getCurrentDocument(frameContext));

    const getCookie = (name, frameContext = currentFrameContext) =>
      getAllCookies(frameContext).find((cookie) => cookie.name === name) || null;

    const addCookie = (cookie, frameContext = currentFrameContext) => {
      if (!cookie || typeof cookie !== "object") {
        throw new Error("Invalid cookie payload");
      }
      if (!cookie.name) {
        throw new Error("Cookie name is required");
      }
      const doc = getCurrentDocument(frameContext);
      const parts = [
        `${encodeURIComponent(cookie.name)}=${encodeURIComponent(cookie.value ?? "")}`
      ];
      if (cookie.path) parts.push(`Path=${cookie.path}`);
      if (cookie.domain) parts.push(`Domain=${cookie.domain}`);
      if (cookie.expiry) parts.push(`Expires=${new Date(Number(cookie.expiry) * 1000).toUTCString()}`);
      if (cookie.secure) parts.push("Secure");
      if (cookie.sameSite) parts.push(`SameSite=${cookie.sameSite}`);
      doc.cookie = parts.join("; ");
      return null;
    };

    const deleteCookie = (name, frameContext = currentFrameContext) => {
      const doc = getCurrentDocument(frameContext);
      const expires = "Thu, 01 Jan 1970 00:00:00 GMT";
      doc.cookie = `${encodeURIComponent(name)}=; Expires=${expires}; Path=/`;
      doc.cookie = `${encodeURIComponent(name)}=; Expires=${expires}`;
      return null;
    };

    const deleteAllCookies = (frameContext = currentFrameContext) => {
      getAllCookies(frameContext).forEach((cookie) => {
        deleteCookie(cookie.name, frameContext);
      });
      return null;
    };

    const getAlertText = (frameContext = currentFrameContext) => {
      const targetWindow = getCurrentWindow(frameContext);
      const state = ensureAlertState(targetWindow);
      if (!state.open) {
        throw new Error("No alert is currently open");
      }
      return state.text || "";
    };

    const sendAlertText = (text, frameContext = currentFrameContext) => {
      const targetWindow = getCurrentWindow(frameContext);
      const state = ensureAlertState(targetWindow);
      if (!state.open) {
        throw new Error("No alert is currently open");
      }
      if (state.type !== "prompt") {
        throw new Error("Alert does not accept text");
      }
      state.promptText = text == null ? null : String(text);
      return null;
    };

    const closeAlert = (accepted, frameContext = currentFrameContext) => {
      const targetWindow = getCurrentWindow(frameContext);
      const state = ensureAlertState(targetWindow);
      if (!state.open) {
        throw new Error("No alert is currently open");
      }
      const result = {
        accepted: !!accepted,
        promptText: state.promptText
      };
      state.open = false;
      state.type = null;
      state.text = "";
      state.defaultValue = null;
      state.promptText = null;
      return result;
    };

    const toFunction = (script, targetWindow) => {
      const trimmed = String(script || "").trim();
      if (!trimmed) {
        return () => null;
      }

      try {
        return targetWindow.eval(`(${trimmed})`);
      } catch (_error) {
        return targetWindow.Function(trimmed);
      }
    };

    const run = async (requestId, script, args, asyncMode, frameContext) => {
      patchConsole();
      try {
        setFrameContext(frameContext);
        const targetWindow = getCurrentWindow(frameContext);
        patchDialogs(targetWindow);
        const fn = toFunction(script, targetWindow);
        const resolvedArgs = deserialize(args);
        let value;
        if (asyncMode) {
          value = await new Promise((resolve, reject) => {
            const callback = (result) => resolve(result);
            try {
              fn.apply(targetWindow, [...resolvedArgs, callback]);
            } catch (error) {
              reject(error);
            }
          });
        } else {
          value = await fn.apply(targetWindow, resolvedArgs);
        }
        await emitResult({
          requestId,
          ok: true,
          value: serialize(value)
        });
      } catch (error) {
        await emitResult({
          requestId,
          ok: false,
          error: {
            name: error && error.name ? error.name : "Error",
            message: error && error.message ? error.message : String(error),
            stack: error && error.stack ? error.stack : null
          }
        });
      }
    };

    patchConsole();

    return {
      getElement,
      getCurrentWindow,
      getCurrentDocument,
      findElements,
      findElementsFromShadow,
      validateFrameByIndex,
      validateFrameElement,
      getShadowRoot,
      isDisplayed,
      clearElement,
      setElementText,
      dispatchPointerClick,
      performActions,
      getAllCookies,
      getCookie,
      addCookie,
      deleteCookie,
      deleteAllCookies,
      getAlertText,
      sendAlertText,
      closeAlert,
      takeLogs,
      run
    };
  })();
}
"#
}
