use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tauri::{Runtime, Webview};

use crate::server::response::WebDriverErrorResponse;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ElementScreenshotMetadata {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(rename = "devicePixelRatio", default = "default_dpr")]
    pub device_pixel_ratio: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrintOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orientation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "pageWidth")]
    pub page_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "pageHeight")]
    pub page_height: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "marginTop")]
    pub margin_top: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "marginBottom")]
    pub margin_bottom: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "marginLeft")]
    pub margin_left: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "marginRight")]
    pub margin_right: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "shrinkToFit")]
    pub shrink_to_fit: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "pageRanges")]
    pub page_ranges: Option<Vec<String>>,
}

fn default_dpr() -> f64 {
    1.0
}

pub async fn take_screenshot<R: Runtime>(
    webview: Webview<R>,
    timeout_ms: u64,
) -> Result<String, WebDriverErrorResponse> {
    imp::take_screenshot(webview, timeout_ms).await
}

pub async fn print_page<R: Runtime>(
    webview: Webview<R>,
    timeout_ms: u64,
    options: &PrintOptions,
) -> Result<String, WebDriverErrorResponse> {
    imp::print_page(webview, timeout_ms, options).await
}

pub fn crop_screenshot(
    screenshot_base64: String,
    metadata: ElementScreenshotMetadata,
) -> Result<String, WebDriverErrorResponse> {
    let png_bytes = BASE64_STANDARD.decode(screenshot_base64).map_err(|error| {
        WebDriverErrorResponse::unknown_error(format!("Invalid PNG payload: {error}"))
    })?;
    let image = image::load_from_memory(&png_bytes).map_err(|error| {
        WebDriverErrorResponse::unknown_error(format!("Failed to decode screenshot PNG: {error}"))
    })?;

    let scale = if metadata.device_pixel_ratio.is_finite() && metadata.device_pixel_ratio > 0.0 {
        metadata.device_pixel_ratio
    } else {
        1.0
    };

    let x = (metadata.x * scale).floor().max(0.0) as u32;
    let y = (metadata.y * scale).floor().max(0.0) as u32;
    let width = (metadata.width * scale).ceil().max(1.0) as u32;
    let height = (metadata.height * scale).ceil().max(1.0) as u32;

    let image_width = image.width();
    let image_height = image.height();
    if x >= image_width || y >= image_height {
        return Err(WebDriverErrorResponse::unknown_error(
            "Element screenshot rectangle is outside the viewport",
        ));
    }

    let clamped_width = width.min(image_width.saturating_sub(x)).max(1);
    let clamped_height = height.min(image_height.saturating_sub(y)).max(1);
    let cropped = image.crop_imm(x, y, clamped_width, clamped_height);

    let mut png = std::io::Cursor::new(Vec::new());
    cropped
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|error| {
            WebDriverErrorResponse::unknown_error(format!("Failed to encode cropped PNG: {error}"))
        })?;

    Ok(BASE64_STANDARD.encode(png.into_inner()))
}

#[cfg(target_os = "macos")]
mod imp {
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use block2::RcBlock;
    use objc2::runtime::AnyObject;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSBitmapImageFileType, NSBitmapImageRep, NSBitmapImageRepPropertyKey, NSImage,
    };
    use objc2_foundation::{NSData, NSDictionary, NSError};
    use objc2_web_kit::{WKPDFConfiguration, WKSnapshotConfiguration, WKWebView};
    use tokio::sync::oneshot;

    pub async fn take_screenshot<R: Runtime>(
        webview: Webview<R>,
        timeout_ms: u64,
    ) -> Result<String, WebDriverErrorResponse> {
        let (tx, rx) = oneshot::channel();

        let result = webview.with_webview(move |platform_webview| unsafe {
            let wk_webview: &WKWebView = &*platform_webview.inner().cast();
            let mtm = MainThreadMarker::new_unchecked();
            let config = WKSnapshotConfiguration::new(mtm);

            let tx = Arc::new(std::sync::Mutex::new(Some(tx)));
            let block = RcBlock::new(move |image: *mut NSImage, error: *mut NSError| {
                let response = if !error.is_null() {
                    let error_ref = &*error;
                    Err(error_ref.localizedDescription().to_string())
                } else if image.is_null() {
                    Err("No image returned".to_string())
                } else {
                    image_to_png_base64(&*image)
                };

                if let Ok(mut guard) = tx.lock() {
                    if let Some(sender) = guard.take() {
                        let _ = sender.send(response);
                    }
                }
            });

            wk_webview.takeSnapshotWithConfiguration_completionHandler(Some(&config), &block);
        });

        if let Err(error) = result {
            return Err(WebDriverErrorResponse::unknown_error(format!(
                "Failed to capture screenshot: {error}"
            )));
        }

        await_base64_response(rx, timeout_ms, "Screenshot").await
    }

    pub async fn print_page<R: Runtime>(
        webview: Webview<R>,
        timeout_ms: u64,
        options: &PrintOptions,
    ) -> Result<String, WebDriverErrorResponse> {
        let page_width = options.page_width.unwrap_or(21.0);
        let page_height = options.page_height.unwrap_or(29.7);
        let margin_top = options.margin_top.unwrap_or(1.0);
        let margin_bottom = options.margin_bottom.unwrap_or(1.0);
        let margin_left = options.margin_left.unwrap_or(1.0);
        let margin_right = options.margin_right.unwrap_or(1.0);
        let orientation = options.orientation.as_deref().unwrap_or("portrait");
        let css = format!(
            r#"(function() {{
                let style = document.getElementById('__bitfun_webdriver_print_style');
                if (!style) {{
                    style = document.createElement('style');
                    style.id = '__bitfun_webdriver_print_style';
                    document.head.appendChild(style);
                }}
                style.textContent = `
                    @page {{
                        size: {page_width}cm {page_height}cm {orientation};
                        margin: {margin_top}cm {margin_right}cm {margin_bottom}cm {margin_left}cm;
                    }}
                    @media print {{
                        body {{
                            -webkit-print-color-adjust: exact;
                            print-color-adjust: exact;
                        }}
                    }}
                `;
            }})();"#
        );
        webview.eval(&css).map_err(|error| {
            WebDriverErrorResponse::unknown_error(format!("Failed to inject print CSS: {error}"))
        })?;

        let (tx, rx) = oneshot::channel();
        let result = webview.with_webview(move |platform_webview| unsafe {
            let wk_webview: &WKWebView = &*platform_webview.inner().cast();
            let mtm = MainThreadMarker::new_unchecked();
            let config = WKPDFConfiguration::new(mtm);

            let tx = Arc::new(std::sync::Mutex::new(Some(tx)));
            let block = RcBlock::new(move |data: *mut NSData, error: *mut NSError| {
                let response = if !error.is_null() {
                    let error_ref = &*error;
                    Err(error_ref.localizedDescription().to_string())
                } else if data.is_null() {
                    Err("No PDF data returned".to_string())
                } else {
                    Ok(BASE64_STANDARD.encode((&*data).to_vec()))
                };

                if let Ok(mut guard) = tx.lock() {
                    if let Some(sender) = guard.take() {
                        let _ = sender.send(response);
                    }
                }
            });

            wk_webview.createPDFWithConfiguration_completionHandler(Some(&config), &block);
        });

        if let Err(error) = result {
            return Err(WebDriverErrorResponse::unknown_error(format!(
                "Failed to print page: {error}"
            )));
        }

        let response = await_base64_response(rx, timeout_ms, "Print").await;
        let _ = webview.eval(
            "(() => { document.getElementById('__bitfun_webdriver_print_style')?.remove(); })();",
        );
        response
    }

    async fn await_base64_response(
        rx: oneshot::Receiver<Result<String, String>>,
        timeout_ms: u64,
        label: &str,
    ) -> Result<String, WebDriverErrorResponse> {
        match tokio::time::timeout(Duration::from_millis(timeout_ms), rx).await {
            Ok(Ok(Ok(base64))) if !base64.is_empty() => Ok(base64),
            Ok(Ok(Ok(_))) => Err(WebDriverErrorResponse::unknown_error(format!(
                "{label} returned empty data"
            ))),
            Ok(Ok(Err(error))) => Err(WebDriverErrorResponse::unknown_error(error)),
            Ok(Err(_)) => Err(WebDriverErrorResponse::unknown_error(format!(
                "{label} channel closed unexpectedly"
            ))),
            Err(_) => Err(WebDriverErrorResponse::timeout(format!(
                "{label} timed out after {timeout_ms}ms"
            ))),
        }
    }

    unsafe fn image_to_png_base64(image: &NSImage) -> Result<String, String> {
        let tiff_data: Option<objc2::rc::Retained<NSData>> = image.TIFFRepresentation();
        let tiff_data = tiff_data.ok_or("Failed to get TIFF representation")?;

        let bitmap_rep = NSBitmapImageRep::imageRepWithData(&tiff_data)
            .ok_or("Failed to create bitmap image rep")?;

        let empty_dict: objc2::rc::Retained<NSDictionary<NSBitmapImageRepPropertyKey, AnyObject>> =
            NSDictionary::new();
        let png_data = bitmap_rep
            .representationUsingType_properties(NSBitmapImageFileType::PNG, &empty_dict)
            .ok_or("Failed to convert image to PNG")?;

        Ok(BASE64_STANDARD.encode(png_data.to_vec()))
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use tokio::sync::oneshot;
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2CapturePreviewCompletedHandler,
        ICoreWebView2CapturePreviewCompletedHandler_Impl, ICoreWebView2Environment6,
        ICoreWebView2PrintToPdfCompletedHandler, ICoreWebView2PrintToPdfCompletedHandler_Impl,
        ICoreWebView2_7, COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
        COREWEBVIEW2_PRINT_ORIENTATION_LANDSCAPE, COREWEBVIEW2_PRINT_ORIENTATION_PORTRAIT,
    };
    use windows::core::{implement, HSTRING};
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::Com::StructuredStorage::CreateStreamOnHGlobal;
    use windows::Win32::System::Com::{
        CoInitializeEx, COINIT_APARTMENTTHREADED, STATFLAG_NONAME, STREAM_SEEK_SET,
    };
    use windows_core::BOOL;

    type CaptureSender = Arc<std::sync::Mutex<Option<oneshot::Sender<Result<String, String>>>>>;
    type PrintSender = Arc<std::sync::Mutex<Option<oneshot::Sender<Result<(), String>>>>>;

    pub async fn take_screenshot<R: Runtime>(
        webview: Webview<R>,
        timeout_ms: u64,
    ) -> Result<String, WebDriverErrorResponse> {
        let (tx, rx) = oneshot::channel();

        let result = webview.with_webview(move |platform_webview| unsafe {
            let webview2 = match platform_webview.controller().CoreWebView2() {
                Ok(webview2) => webview2,
                Err(error) => {
                    let _ = tx.send(Err(format!("Failed to access CoreWebView2: {error:?}")));
                    return;
                }
            };

            let stream = match CreateStreamOnHGlobal(HGLOBAL::default(), true) {
                Ok(stream) => stream,
                Err(error) => {
                    let _ = tx.send(Err(format!("Failed to create preview stream: {error}")));
                    return;
                }
            };

            let handler_tx = Arc::new(std::sync::Mutex::new(Some(tx)));
            let handler = CapturePreviewHandler::new(handler_tx, stream.clone());
            let handler: ICoreWebView2CapturePreviewCompletedHandler = handler.into();

            if let Err(error) = webview2.CapturePreview(
                COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
                &stream,
                &handler,
            ) {
                if let Ok(mut guard) = handler_tx.lock() {
                    if let Some(tx) = guard.take() {
                        let _ = tx.send(Err(format!("CapturePreview failed: {error:?}")));
                    }
                }
            }
        });

        if let Err(error) = result {
            return Err(WebDriverErrorResponse::unknown_error(format!(
                "Failed to capture screenshot: {error}"
            )));
        }

        await_base64_response(rx, timeout_ms, "Screenshot").await
    }

    pub async fn print_page<R: Runtime>(
        webview: Webview<R>,
        timeout_ms: u64,
        options: &PrintOptions,
    ) -> Result<String, WebDriverErrorResponse> {
        let (tx, rx) = oneshot::channel();
        let tx = Arc::new(std::sync::Mutex::new(Some(tx)));

        let temp_dir = tempfile::TempDir::new().map_err(|error| {
            WebDriverErrorResponse::unknown_error(format!("Failed to create temp dir: {error}"))
        })?;
        let pdf_path = temp_dir.path().join("print.pdf");
        let pdf_path_clone = pdf_path.clone();

        let orientation = options.orientation.clone();
        let scale = options.scale;
        let background = options.background;
        let page_width = options.page_width;
        let page_height = options.page_height;
        let margin_top = options.margin_top;
        let margin_bottom = options.margin_bottom;
        let margin_left = options.margin_left;
        let margin_right = options.margin_right;

        let result = webview.with_webview(move |platform_webview| unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            let webview2 = match platform_webview.controller().CoreWebView2() {
                Ok(webview2) => webview2,
                Err(error) => {
                    if let Ok(mut guard) = tx.lock() {
                        if let Some(tx) = guard.take() {
                            let _ =
                                tx.send(Err(format!("Failed to access CoreWebView2: {error:?}")));
                        }
                    }
                    return;
                }
            };

            let webview7: ICoreWebView2_7 = match webview2.cast() {
                Ok(webview7) => webview7,
                Err(error) => {
                    if let Ok(mut guard) = tx.lock() {
                        if let Some(tx) = guard.take() {
                            let _ = tx.send(Err(format!(
                                "Failed to cast CoreWebView2 to ICoreWebView2_7: {error:?}"
                            )));
                        }
                    }
                    return;
                }
            };

            let environment = match webview7.Environment() {
                Ok(environment) => environment,
                Err(error) => {
                    if let Ok(mut guard) = tx.lock() {
                        if let Some(tx) = guard.take() {
                            let _ = tx.send(Err(format!(
                                "Failed to access WebView2 environment: {error:?}"
                            )));
                        }
                    }
                    return;
                }
            };

            let env6: ICoreWebView2Environment6 = match environment.cast() {
                Ok(env6) => env6,
                Err(error) => {
                    if let Ok(mut guard) = tx.lock() {
                        if let Some(tx) = guard.take() {
                            let _ = tx.send(Err(format!(
                                "Failed to cast environment to ICoreWebView2Environment6: {error:?}"
                            )));
                        }
                    }
                    return;
                }
            };

            let settings = match env6.CreatePrintSettings() {
                Ok(settings) => settings,
                Err(error) => {
                    if let Ok(mut guard) = tx.lock() {
                        if let Some(tx) = guard.take() {
                            let _ =
                                tx.send(Err(format!("Failed to create print settings: {error:?}")));
                        }
                    }
                    return;
                }
            };

            if let Some(orientation) = orientation.as_deref() {
                let orientation_value = if orientation == "landscape" {
                    COREWEBVIEW2_PRINT_ORIENTATION_LANDSCAPE
                } else {
                    COREWEBVIEW2_PRINT_ORIENTATION_PORTRAIT
                };
                let _ = settings.SetOrientation(orientation_value);
            }
            if let Some(scale) = scale {
                let _ = settings.SetScaleFactor(scale);
            }
            if let Some(background) = background {
                let _ = settings.SetShouldPrintBackgrounds(background);
            }
            if let Some(page_width) = page_width {
                let _ = settings.SetPageWidth(page_width / 2.54);
            }
            if let Some(page_height) = page_height {
                let _ = settings.SetPageHeight(page_height / 2.54);
            }
            if let Some(margin_top) = margin_top {
                let _ = settings.SetMarginTop(margin_top / 2.54);
            }
            if let Some(margin_bottom) = margin_bottom {
                let _ = settings.SetMarginBottom(margin_bottom / 2.54);
            }
            if let Some(margin_left) = margin_left {
                let _ = settings.SetMarginLeft(margin_left / 2.54);
            }
            if let Some(margin_right) = margin_right {
                let _ = settings.SetMarginRight(margin_right / 2.54);
            }

            let handler_tx = tx.clone();
            let handler: ICoreWebView2PrintToPdfCompletedHandler =
                PrintToPdfHandler::new(tx).into();
            let path = HSTRING::from(pdf_path_clone.to_string_lossy().to_string());

            if let Err(error) = webview7.PrintToPdf(&path, &settings, &handler) {
                // PrintToPdf won't invoke the callback when the call itself fails.
                if let Ok(mut guard) = handler_tx.lock() {
                    if let Some(tx) = guard.take() {
                        let _ = tx.send(Err(format!("PrintToPdf call failed: {error:?}")));
                    }
                }
            }
        });

        if let Err(error) = result {
            return Err(WebDriverErrorResponse::unknown_error(format!(
                "Failed to print page: {error}"
            )));
        }

        match tokio::time::timeout(Duration::from_millis(timeout_ms), rx).await {
            Ok(Ok(Ok(()))) => {}
            Ok(Ok(Err(error))) => return Err(WebDriverErrorResponse::unknown_error(error)),
            Ok(Err(_)) => {
                return Err(WebDriverErrorResponse::unknown_error(
                    "Print channel closed unexpectedly",
                ))
            }
            Err(_) => {
                return Err(WebDriverErrorResponse::timeout(format!(
                    "Print timed out after {timeout_ms}ms"
                )))
            }
        }

        let pdf_bytes = std::fs::read(&pdf_path).map_err(|error| {
            WebDriverErrorResponse::unknown_error(format!("Failed to read printed PDF: {error}"))
        })?;
        Ok(BASE64_STANDARD.encode(pdf_bytes))
    }

    async fn await_base64_response(
        rx: oneshot::Receiver<Result<String, String>>,
        timeout_ms: u64,
        label: &str,
    ) -> Result<String, WebDriverErrorResponse> {
        match tokio::time::timeout(Duration::from_millis(timeout_ms), rx).await {
            Ok(Ok(Ok(base64))) if !base64.is_empty() => Ok(base64),
            Ok(Ok(Ok(_))) => Err(WebDriverErrorResponse::unknown_error(format!(
                "{label} returned empty data"
            ))),
            Ok(Ok(Err(error))) => Err(WebDriverErrorResponse::unknown_error(error)),
            Ok(Err(_)) => Err(WebDriverErrorResponse::unknown_error(format!(
                "{label} channel closed unexpectedly"
            ))),
            Err(_) => Err(WebDriverErrorResponse::timeout(format!(
                "{label} timed out after {timeout_ms}ms"
            ))),
        }
    }

    #[implement(ICoreWebView2CapturePreviewCompletedHandler)]
    struct CapturePreviewHandler {
        tx: CaptureSender,
        stream: windows::Win32::System::Com::IStream,
    }

    impl CapturePreviewHandler {
        fn new(tx: CaptureSender, stream: windows::Win32::System::Com::IStream) -> Self {
            Self { tx, stream }
        }
    }

    impl ICoreWebView2CapturePreviewCompletedHandler_Impl for CapturePreviewHandler_Impl {
        fn Invoke(&self, errorcode: windows::core::HRESULT) -> windows::core::Result<()> {
            let response = if errorcode.is_err() {
                Err(format!("Capture preview failed: {errorcode:?}"))
            } else {
                unsafe {
                    let mut stat = std::mem::zeroed();
                    if self.stream.Stat(&raw mut stat, STATFLAG_NONAME).is_err() {
                        return Ok(());
                    }
                    let size = usize::try_from(stat.cbSize).unwrap_or(0);
                    if size == 0 {
                        Err("Capture preview returned empty data".to_string())
                    } else {
                        let _ = self.stream.Seek(0, STREAM_SEEK_SET, None);
                        let mut buffer = vec![0u8; size];
                        let mut bytes_read = 0u32;
                        if self
                            .stream
                            .Read(
                                buffer.as_mut_ptr().cast(),
                                u32::try_from(size).unwrap_or(u32::MAX),
                                Some(&raw mut bytes_read),
                            )
                            .is_err()
                        {
                            Err("Failed to read preview stream".to_string())
                        } else {
                            buffer.truncate(bytes_read as usize);
                            Ok(BASE64_STANDARD.encode(buffer))
                        }
                    }
                }
            };

            if let Ok(mut guard) = self.tx.lock() {
                if let Some(tx) = guard.take() {
                    let _ = tx.send(response);
                }
            }
            Ok(())
        }
    }

    #[implement(ICoreWebView2PrintToPdfCompletedHandler)]
    struct PrintToPdfHandler {
        tx: PrintSender,
    }

    impl PrintToPdfHandler {
        fn new(tx: PrintSender) -> Self {
            Self { tx }
        }
    }

    impl ICoreWebView2PrintToPdfCompletedHandler_Impl for PrintToPdfHandler_Impl {
        fn Invoke(
            &self,
            errorcode: windows::core::HRESULT,
            issuccessful: BOOL,
        ) -> windows::core::Result<()> {
            let response = if errorcode.is_err() {
                Err(format!("PrintToPdf failed: {errorcode:?}"))
            } else if !issuccessful.as_bool() {
                Err("PrintToPdf was not successful".to_string())
            } else {
                Ok(())
            };

            if let Ok(mut guard) = self.tx.lock() {
                if let Some(tx) = guard.take() {
                    let _ = tx.send(response);
                }
            }
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use glib::MainContext;
    use gtk::cairo::ImageSurface;
    use tokio::sync::oneshot;
    use webkit2gtk::{PrintOperationExt, SnapshotOptions, SnapshotRegion, WebViewExt};

    pub async fn take_screenshot<R: Runtime>(
        webview: Webview<R>,
        timeout_ms: u64,
    ) -> Result<String, WebDriverErrorResponse> {
        let (tx, rx) = oneshot::channel();

        let result = webview.with_webview(move |platform_webview| {
            let webview = platform_webview.inner().clone();
            let tx = Arc::new(std::sync::Mutex::new(Some(tx)));
            let context = MainContext::default();

            context.spawn_local(async move {
                let response = match webview
                    .snapshot_future(SnapshotRegion::Visible, SnapshotOptions::NONE)
                    .await
                {
                    Ok(surface) => {
                        let mut png_bytes = Vec::new();
                        match ImageSurface::try_from(surface) {
                            Ok(image_surface) => match image_surface.write_to_png(&mut png_bytes) {
                                Ok(()) => Ok(BASE64_STANDARD.encode(png_bytes)),
                                Err(error) => Err(format!("Failed to encode PNG: {error}")),
                            },
                            Err(error) => {
                                Err(format!("Failed to convert snapshot surface: {error:?}"))
                            }
                        }
                    }
                    Err(error) => Err(error.to_string()),
                };

                if let Ok(mut guard) = tx.lock() {
                    if let Some(tx) = guard.take() {
                        let _ = tx.send(response);
                    }
                }
            });
        });

        if let Err(error) = result {
            return Err(WebDriverErrorResponse::unknown_error(format!(
                "Failed to capture screenshot: {error}"
            )));
        }

        await_base64_response(rx, timeout_ms, "Screenshot").await
    }

    pub async fn print_page<R: Runtime>(
        webview: Webview<R>,
        timeout_ms: u64,
        options: &PrintOptions,
    ) -> Result<String, WebDriverErrorResponse> {
        let (tx, rx) = oneshot::channel::<Result<(), String>>();

        let temp_dir = tempfile::TempDir::new().map_err(|error| {
            WebDriverErrorResponse::unknown_error(format!("Failed to create temp dir: {error}"))
        })?;
        let pdf_path = temp_dir.path().join("print.pdf");
        let pdf_path_clone = pdf_path.clone();

        let orientation = options.orientation.clone();
        let page_width = options.page_width;
        let page_height = options.page_height;
        let margin_top = options.margin_top;
        let margin_bottom = options.margin_bottom;
        let margin_left = options.margin_left;
        let margin_right = options.margin_right;

        let result = webview.with_webview(move |platform_webview| {
            let webview = platform_webview.inner().clone();
            let print_operation = webkit2gtk::PrintOperation::new(&webview);
            let page_setup = gtk::PageSetup::new();

            let width_points = page_width.unwrap_or(21.0) * 28.35;
            let height_points = page_height.unwrap_or(29.7) * 28.35;
            let paper_size = gtk::PaperSize::new_custom(
                "custom",
                "Custom",
                width_points,
                height_points,
                gtk::Unit::Points,
            );
            page_setup.set_paper_size(&paper_size);

            if orientation.as_deref() == Some("landscape") {
                page_setup.set_orientation(gtk::PageOrientation::Landscape);
            } else {
                page_setup.set_orientation(gtk::PageOrientation::Portrait);
            }

            page_setup.set_top_margin(margin_top.unwrap_or(1.0) * 28.35, gtk::Unit::Points);
            page_setup.set_bottom_margin(margin_bottom.unwrap_or(1.0) * 28.35, gtk::Unit::Points);
            page_setup.set_left_margin(margin_left.unwrap_or(1.0) * 28.35, gtk::Unit::Points);
            page_setup.set_right_margin(margin_right.unwrap_or(1.0) * 28.35, gtk::Unit::Points);
            print_operation.set_page_setup(&page_setup);

            let settings = gtk::PrintSettings::new();
            settings.set_printer("Print to File");
            settings.set(
                gtk::PRINT_SETTINGS_OUTPUT_URI,
                Some(&format!("file://{}", pdf_path_clone.display())),
            );
            settings.set(gtk::PRINT_SETTINGS_OUTPUT_FILE_FORMAT, Some("pdf"));
            print_operation.set_print_settings(&settings);

            let tx = Arc::new(std::sync::Mutex::new(Some(tx)));
            print_operation.connect_finished(move |_operation| {
                if let Ok(mut guard) = tx.lock() {
                    if let Some(tx) = guard.take() {
                        let _ = tx.send(Ok(()));
                    }
                }
            });

            let () = print_operation.print();
        });

        if let Err(error) = result {
            return Err(WebDriverErrorResponse::unknown_error(format!(
                "Failed to print page: {error}"
            )));
        }

        match tokio::time::timeout(Duration::from_millis(timeout_ms), rx).await {
            Ok(Ok(Ok(()))) => {}
            Ok(Ok(Err(error))) => return Err(WebDriverErrorResponse::unknown_error(error)),
            Ok(Err(_)) => {
                return Err(WebDriverErrorResponse::unknown_error(
                    "Print channel closed unexpectedly",
                ))
            }
            Err(_) => {
                return Err(WebDriverErrorResponse::timeout(format!(
                    "Print timed out after {timeout_ms}ms"
                )))
            }
        }

        let pdf_bytes = std::fs::read(&pdf_path).map_err(|error| {
            WebDriverErrorResponse::unknown_error(format!("Failed to read printed PDF: {error}"))
        })?;
        Ok(BASE64_STANDARD.encode(pdf_bytes))
    }

    async fn await_base64_response(
        rx: oneshot::Receiver<Result<String, String>>,
        timeout_ms: u64,
        label: &str,
    ) -> Result<String, WebDriverErrorResponse> {
        match tokio::time::timeout(Duration::from_millis(timeout_ms), rx).await {
            Ok(Ok(Ok(base64))) if !base64.is_empty() => Ok(base64),
            Ok(Ok(Ok(_))) => Err(WebDriverErrorResponse::unknown_error(format!(
                "{label} returned empty data"
            ))),
            Ok(Ok(Err(error))) => Err(WebDriverErrorResponse::unknown_error(error)),
            Ok(Err(_)) => Err(WebDriverErrorResponse::unknown_error(format!(
                "{label} channel closed unexpectedly"
            ))),
            Err(_) => Err(WebDriverErrorResponse::timeout(format!(
                "{label} timed out after {timeout_ms}ms"
            ))),
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod imp {
    use super::*;

    pub async fn take_screenshot<R: Runtime>(
        _webview: Webview<R>,
        _timeout_ms: u64,
    ) -> Result<String, WebDriverErrorResponse> {
        Err(WebDriverErrorResponse::unknown_error(
            "Native screenshot is not implemented for this platform yet",
        ))
    }

    pub async fn print_page<R: Runtime>(
        _webview: Webview<R>,
        _timeout_ms: u64,
        _options: &PrintOptions,
    ) -> Result<String, WebDriverErrorResponse> {
        Err(WebDriverErrorResponse::unsupported_operation(
            "Printing is not implemented for this platform yet",
        ))
    }
}
