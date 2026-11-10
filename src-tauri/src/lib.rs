//! IP Monitor desktop shell.
//!
//! Phase 1 was window-only. This revision adds the parts of the shell that are
//! not the card itself: persisted preferences, the always-on-top toggle and the
//! tray icon. Phase 2 adds `commands::get_ip_info` behind an `IpInfoProvider`
//! seam (spec §5.5) and registers it in `invoke_handler` below.

mod detect;
mod edge;
mod proxy;
mod secret;
mod settings;

use std::sync::Mutex;

use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_autostart::ManagerExt as AutostartExt;

use crate::detect::{DetectionError, DetectionResult};
use crate::proxy::{CustomProxyConfig, DetectionMode};
use crate::settings::{CustomProxySettings, Settings, SettingsState, Theme};

/// Tray menu item kept alive so the checkbox can mirror the window state.
struct TrayHandles {
    pin_item: CheckMenuItem<Wry>,
}

fn show_card(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    // Showing a parked tab means unfolding it first, otherwise the user gets an
    // unclickable sliver and has to guess.
    expand_from_edge(app, &window);
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

fn persist(app: &AppHandle, settings: &Settings) {
    if let Err(error) = settings::save(app, settings) {
        eprintln!("settings: cannot persist ({error})");
    }
}

/// Apply the pin state, persist it and mirror it into the tray checkbox.
///
/// Returns the state the window actually ended up in: the command contract is
/// "tell me what really happened", never "assume the request succeeded".
fn apply_pin(app: &AppHandle, pinned: bool) -> bool {
    let Some(window) = app.get_webview_window("main") else {
        return pinned;
    };
    if let Err(error) = window.set_always_on_top(pinned) {
        eprintln!("window: cannot change always-on-top ({error})");
        return !pinned;
    }

    let effective = window.is_always_on_top().unwrap_or(pinned);
    let updated = app.state::<SettingsState>().update(|s| s.pinned = effective);
    persist(app, &updated);
    let _ = app.emit("settings-changed", &updated);

    if let Some(handles) = app.try_state::<TrayHandles>() {
        let _ = handles.pin_item.set_checked(effective);
    }
    effective
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "show" => show_card(app),
        "pin" => {
            let next = !app.state::<SettingsState>().pinned();
            apply_pin(app, next);
        }
        "quit" => app.exit(0),
        _ => {}
    }
}

fn build_tray(app: &AppHandle, pinned: bool) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, "show", "显示卡片", true, None::<&str>)?;
    let pin_item = CheckMenuItem::with_id(app, "pin", "置顶显示", true, pinned, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出 IP Monitor", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&show_item, &pin_item, &separator, &quit_item])?;

    TrayIconBuilder::with_id("main")
        .icon(tauri::image::Image::from_bytes(include_bytes!(
            "../icons/tray-32.png"
        ))?)
        .tooltip("IP Monitor")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_card(tray.app_handle());
            }
        })
        .build(app)?;

    app.manage(TrayHandles { pin_item });
    Ok(())
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, SettingsState>) -> Settings {
    state.snapshot()
}

#[tauri::command]
fn set_pinned(app: AppHandle, pinned: bool) -> bool {
    apply_pin(&app, pinned)
}

#[tauri::command]
fn set_size_locked(app: AppHandle, locked: bool) -> Result<bool, String> {
    let window = app.get_webview_window("main").ok_or("main window unavailable")?;
    let state = app.state::<SettingsState>();
    let mut updated = state.snapshot();
    if updated.size_locked == locked {
        return Ok(locked);
    }

    let parked = app.state::<edge::EdgeState>().is_parked();
    window.set_resizable(!locked && !parked).map_err(|error| error.to_string())?;
    updated.size_locked = locked;
    if let Err(error) = settings::save(&app, &updated) {
        let _ = window.set_resizable(locked && !parked);
        return Err(error.to_string());
    }
    let updated = state.update(|settings| settings.size_locked = locked);
    let _ = app.emit("settings-changed", &updated);
    Ok(locked)
}

#[tauri::command]
fn set_theme(app: AppHandle, theme: Theme) -> Settings {
    let updated = app.state::<SettingsState>().update(|s| s.theme = theme);
    persist(&app, &updated);
    let _ = app.emit("settings-changed", &updated);
    updated
}

/// Launch-at-login lives in the OS (the Run key), never in settings.json:
/// the registry is the only honest source of truth for it.
#[tauri::command]
fn get_autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> bool {
    let manager = app.autolaunch();
    let outcome = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    if let Err(error) = outcome {
        eprintln!("autostart: cannot change ({error})");
    }
    // Report what the OS actually says, including after a failed write.
    manager.is_enabled().unwrap_or(!enabled)
}

/// Whether to ask the compositor for a material behind the card.
///
/// Every route we can reach on this Windows build misbehaves: the legacy
/// accent policy (`ACCENT_ENABLE_BLURBEHIND`) leaves a grey veil over the card
/// and does not blur anything, and the Win11 system backdrops
/// (`DWMSBT_TRANSIENTWINDOW` / acrylic) composite as a nearly opaque milky
/// sheet. Both read worse than a plain translucent fill, so the CSS fill is
/// the whole material for now and this stays off. Flip it back when there is a
/// path that actually blurs the desktop.
const COMPOSITOR_MATERIAL: bool = false;

/// Turn on real background blur behind the card.
///
/// CSS `backdrop-filter` only blurs the page's own content, never the desktop,
/// so the frosted look has to come from the compositor. The Win11 system
/// backdrop (`DWMSBT_TRANSIENTWINDOW`) is the documented path, but on a layered
/// transparent window it composites as a nearly opaque tint here — it hides the
/// desktop instead of blurring it. The legacy accent policy samples what is
/// actually behind the window, which is what this card is for.
#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn apply_material(window: &tauri::WebviewWindow) {
    if let Err(error) = window_vibrancy::apply_blur(window, None) {
        // Spec §10.4: material failures degrade, they never block the window.
        eprintln!("material: blur unavailable ({error})");
    }
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
fn apply_material(_window: &tauri::WebviewWindow) {}

/// Corner radius of the card, in logical pixels. The CSS token of the same
/// name has to match, or the painted corner and the window agree on a different
/// shape.
const CARD_CORNER: f64 = 28.0;

/// Corner radius of the folded tab, in logical pixels (matches `.tab`).
const TAB_CORNER: f64 = 8.0;

/// How much smaller the window region is than the painted corner.
///
/// A window region is a bitmap: its arc is a staircase with no anti-aliasing. If
/// the region and the CSS corner have the same radius, that staircase lands right
/// on the card's own anti-aliased curve and the edge looks chewed. Pulling the
/// region in by a few pixels moves the staircase into the fully transparent band
/// outside the card, so what the eye sees is the CSS curve again — while the
/// region still keeps the corner out of hit-testing.
const REGION_SLACK: f64 = 4.0;

/// Give the window the shape the card paints, through a window region.
///
/// While the compositor material was in use the window had to be rounded with
/// DWM, whose radius is fixed at 8 logical pixels — hence the old 8px card. With
/// the material gone there is nothing left outside the CSS card, so a region can
/// carry any radius we like, and unlike the DWM corner it also clips hit-testing:
/// clicks in the cut-away corner fall through to whatever is behind instead of
/// grabbing an invisible corner of the window.
#[cfg(target_os = "windows")]
fn shape_window(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    use std::ffi::c_void;

    #[link(name = "user32")]
    extern "system" {
        fn GetDpiForWindow(hwnd: *mut c_void) -> u32;
        fn CreateRoundRectRgn(
            left: i32,
            top: i32,
            right: i32,
            bottom: i32,
            ellipse_width: i32,
            ellipse_height: i32,
        ) -> *mut c_void;
        fn SetWindowRgn(hwnd: *mut c_void, region: *mut c_void, redraw: i32) -> i32;
    }

    let Ok(handle) = window.hwnd() else {
        return;
    };
    let hwnd = handle.0 as *mut c_void;

    // `scale_factor()` reports 1.0 until the window has a monitor, so ask the
    // window itself; the radius has to be in device pixels.
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let scale = if dpi > 0 {
        dpi as f64 / 96.0
    } else {
        window.scale_factor().unwrap_or(1.0)
    };

    let parked = app.state::<edge::EdgeState>().is_parked();
    let corner = if parked { TAB_CORNER } else { CARD_CORNER };
    let radius = ((corner - REGION_SLACK) * scale).round().max(1.0) as i32;

    let Ok(size) = window.outer_size() else {
        return;
    };
    let (width, height) = (size.width as i32, size.height as i32);
    if width <= 0 || height <= 0 {
        return;
    }

    unsafe {
        // The ellipse is the full corner (2R across), and the +1 keeps the last
        // row and column of pixels inside the region.
        let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, radius * 2, radius * 2);
        if !region.is_null() {
            // SetWindowRgn owns the region after a successful call.
            SetWindowRgn(hwnd, region, 1);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn shape_window(_app: &tauri::AppHandle, _window: &tauri::WebviewWindow) {}

/// The runtime reapplies the caption style when native window flags change.
/// Strip it before Windows paints the transparent window's non-client frame,
/// including at startup and when the user toggles resizing.
#[cfg(target_os = "windows")]
fn set_frame_constraint(window: &tauri::WebviewWindow, square: i32) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MINMAXINFO, SWP_NOSIZE, WINDOWPOS, WM_GETMINMAXINFO, WM_NCHITTEST, WM_WINDOWPOSCHANGING,
    };
    use std::ffi::c_void;
    const GWL_STYLE: i32 = -16;
    const WM_STYLECHANGING: u32 = 0x007c;
    const WM_NCACTIVATE: u32 = 0x0086;
    const CAPTION: u32 = 0x00c0_0000;
    #[repr(C)]
    struct StyleChange {
        _old: u32,
        new: u32,
    }
    type Callback = unsafe extern "system" fn(*mut c_void, u32, usize, isize, usize, usize) -> isize;
    #[link(name = "comctl32")]
    extern "system" {
        fn SetWindowSubclass(h: *mut c_void, callback: Callback, id: usize, data: usize) -> i32;
        fn DefSubclassProc(h: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> isize;
    }
    #[link(name = "user32")]
    extern "system" {
        fn GetWindowLongW(h: *mut c_void, index: i32) -> i32;
        fn SetWindowLongW(h: *mut c_void, index: i32, value: i32) -> i32;
    }
    unsafe extern "system" fn frame(h: *mut c_void, msg: u32, wparam: usize,
        lparam: isize, _: usize, square: usize) -> isize
    {
        let lparam = match msg {
            WM_STYLECHANGING if wparam as i32 == GWL_STYLE && lparam != 0 => {
                (*(lparam as *mut StyleChange)).new &= !CAPTION;
                lparam
            }
            WM_NCACTIVATE => -1,
            _ => lparam,
        };
        let result = DefSubclassProc(h, msg, wparam, lparam);
        if square != 0 {
            let square = square as i32;
            match msg {
                WM_GETMINMAXINFO if lparam != 0 => {
                    let info = &mut *(lparam as *mut MINMAXINFO);
                    info.ptMinTrackSize.x = square;
                    info.ptMinTrackSize.y = square;
                    info.ptMaxTrackSize = info.ptMinTrackSize;
                }
                WM_WINDOWPOSCHANGING if lparam != 0 => {
                    let pos = &mut *(lparam as *mut WINDOWPOS);
                    if pos.flags & SWP_NOSIZE == 0 {
                        pos.cx = square;
                        pos.cy = square;
                    }
                }
                // HTLEFT through HTBORDER are resize/border hit targets.
                WM_NCHITTEST if (10..=18).contains(&result) => return 1, // HTCLIENT
                _ => {}
            }
        }
        result
    }

    let Ok(hwnd) = window.hwnd() else { return false };
    let hwnd = hwnd.0 as *mut c_void;
    unsafe {
        if SetWindowSubclass(hwnd, frame, 1, square as usize) == 0 {
            eprintln!("window: cannot update native frame constraint");
            return false;
        }
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        if style & CAPTION != 0 {
            SetWindowLongW(hwnd, GWL_STYLE, (style & !CAPTION) as i32);
        }
    }
    true
}

#[cfg(not(target_os = "windows"))]
fn set_frame_constraint(_window: &tauri::WebviewWindow, _square: i32) -> bool { true }

/// Session-only detection state.
///
/// The mode is deliberately *not* persisted: a cold start must always be Direct
/// so the card never silently inherits a Windows proxy (spec §6.1, and the user
/// decision that the mode is remembered for the session only).
#[derive(Default)]
struct DetectionState {
    mode: Mutex<Option<DetectionMode>>,
}

impl DetectionState {
    fn mode(&self) -> DetectionMode {
        let guard = self
            .mode
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.unwrap_or(DetectionMode::Direct)
    }

    fn set_mode(&self, mode: DetectionMode) {
        *self
            .mode
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(mode);
    }
}

/// What the settings dialog gets to see: never the password (spec §6.3).
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CustomProxyView {
    protocol: proxy::ProxyProtocol,
    host: String,
    port: u16,
    username: Option<String>,
    /// Whether a password is stored (encrypted) for this proxy.
    has_password: bool,
}

impl CustomProxyView {
    fn from_settings(settings: CustomProxySettings) -> Self {
        Self {
            protocol: settings.protocol,
            host: settings.host,
            port: settings.port,
            has_password: settings.password_blob.is_some(),
            username: settings.username,
        }
    }
}

/// Assembles the runtime config: the persisted half plus the decrypted secret.
fn custom_proxy_config(app: &AppHandle) -> Option<CustomProxyConfig> {
    let stored = app.state::<SettingsState>().snapshot().custom_proxy?;
    let password = stored
        .password_blob
        .as_deref()
        .and_then(secret::unprotect);
    Some(CustomProxyConfig {
        protocol: stored.protocol,
        host: stored.host,
        port: stored.port,
        username: stored.username,
        password,
    })
}

#[tauri::command]
async fn get_ip_info(
    app: AppHandle,
    mode: Option<DetectionMode>,
) -> Result<DetectionResult, DetectionError> {
    let mode = mode.unwrap_or_else(|| app.state::<DetectionState>().mode());
    // A slow earlier request must not overwrite the selected session mode.
    let custom = custom_proxy_config(&app);
    if mode == DetectionMode::CustomProxy && custom.is_none() {
        return Err(DetectionError::new(
            "not_configured",
            "还没有配置自定义代理，请先填写地址和端口",
        ));
    }

    detect::detect(mode, custom.as_ref()).await
}

#[tauri::command]
fn get_detection_mode(app: AppHandle) -> DetectionMode {
    app.state::<DetectionState>().mode()
}

#[tauri::command]
fn set_detection_mode(app: AppHandle, mode: DetectionMode) -> DetectionMode {
    app.state::<DetectionState>().set_mode(mode);
    let _ = app.emit("detection-mode-changed", mode);
    mode
}

#[tauri::command]
fn get_custom_proxy(app: AppHandle) -> Option<CustomProxyView> {
    app.state::<SettingsState>()
        .snapshot()
        .custom_proxy
        .map(CustomProxyView::from_settings)
}

#[tauri::command]
fn validate_custom_proxy(config: CustomProxyConfig) -> Result<(), DetectionError> {
    config
        .validate()
        .map_err(|error| DetectionError::new("invalid", error.to_string()))
}

#[tauri::command]
fn save_custom_proxy(
    app: AppHandle,
    config: CustomProxyConfig,
) -> Result<CustomProxyView, DetectionError> {
    config
        .validate()
        .map_err(|error| DetectionError::new("invalid", error.to_string()))?;

    let previous = app.state::<SettingsState>().snapshot().custom_proxy;

    // Empty password means "keep the one already stored": the form never shows
    // the old password back, so requiring a retype on every edit would be hostile.
    let password_blob = match config.password.as_deref() {
        Some(password) if !password.is_empty() => match secret::protect(password) {
            Some(blob) => Some(blob),
            None => {
                return Err(DetectionError::new(
                    "secret",
                    "无法加密密码（DPAPI 不可用），没有保存",
                ))
            }
        },
        _ => previous.as_ref().and_then(|settings| settings.password_blob.clone()),
    };

    if config.username.is_some() && password_blob.is_none() {
        return Err(DetectionError::new(
            "invalid",
            "这个代理需要密码：请填写密码后再保存",
        ));
    }

    let stored = CustomProxySettings {
        protocol: config.protocol,
        host: config.host.trim().to_string(),
        port: config.port,
        username: config.username.clone(),
        password_blob,
    };
    let updated = app
        .state::<SettingsState>()
        .update(|settings| settings.custom_proxy = Some(stored.clone()));
    persist(&app, &updated);
    let _ = app.emit("settings-changed", &updated);

    Ok(CustomProxyView::from_settings(stored))
}

/// What the frontend needs to know about the folded state.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct EdgeSnapshot {
    parked: bool,
    side: Option<edge::Side>,
    revision: u64,
    center: i32,
}

impl EdgeSnapshot {
    fn card() -> Self {
        Self {
            parked: false,
            side: None,
            revision: 0,
            center: 0,
        }
    }

    fn tab(parked: edge::Parked, revision: u64) -> Self {
        Self {
            parked: true,
            side: Some(parked.side),
            revision,
            center: edge::along_center(parked.side, parked.tab),
        }
    }
}

/// Apply one window rectangle so the compositor never sees a tab at the old
/// card origin or a full-sized card at the tab origin.
fn set_edge_rect(window: &tauri::WebviewWindow, rect: edge::Rect) -> bool {
    #[cfg(target_os = "windows")]
    if let Ok(hwnd) = window.hwnd() {
        use std::ffi::c_void;
        #[link(name = "user32")]
        extern "system" {
            fn SetWindowPos(hwnd: *mut c_void, after: *mut c_void, x: i32, y: i32,
                width: i32, height: i32, flags: u32) -> i32;
        }
        // SWP_NOZORDER | SWP_NOACTIVATE
        if unsafe { SetWindowPos(hwnd.0 as *mut c_void, std::ptr::null_mut(),
            rect.x, rect.y, rect.width, rect.height, 0x0014) } != 0
        {
            return window_rect(window) == Some(rect);
        }
    }
    window.set_size(tauri::PhysicalSize::new(rect.width as u32, rect.height as u32)).is_ok()
        && window.set_position(tauri::PhysicalPosition::new(rect.x, rect.y)).is_ok()
        && window_rect(window) == Some(rect)
}

#[derive(PartialEq, Eq)]
enum ParkAttempt {
    Skipped,
    Deferred,
    Parked,
}

/// Fold the card into an edge tab if it was just dropped against a monitor edge.
///
fn park_if_near_edge(app: &AppHandle, window: &tauri::WebviewWindow, leave_request: bool) -> ParkAttempt {
    let state = app.state::<edge::EdgeState>();
    if state.is_parked() || (leave_request && !state.is_edge_preview()) {
        return ParkAttempt::Skipped;
    }
    if primary_mouse_down() {
        return ParkAttempt::Deferred;
    }

    let (Ok(position), Ok(size), Ok(scale)) = (
        window.outer_position(),
        window.outer_size(),
        window.scale_factor(),
    ) else {
        return ParkAttempt::Skipped;
    };
    let Some(monitor) = window.current_monitor().ok().flatten() else {
        return ParkAttempt::Skipped;
    };

    let card = edge::Rect {
        x: position.x,
        y: position.y,
        width: size.width as i32,
        height: size.height as i32,
    };
    let origin = monitor.position();
    let extent = monitor.size();
    let monitor_rect = edge::Rect {
        x: origin.x,
        y: origin.y,
        width: extent.width as i32,
        height: extent.height as i32,
    };

    let Some(side) = edge::side_for(card, monitor_rect) else {
        state.end_edge_preview();
        return ParkAttempt::Skipped;
    };
    // A card just expanded from a tab stays open under the pointer, even
    // when a native move or resize has just ended.
    if state.is_edge_preview() && !cursor_outside(window, card) {
        return ParkAttempt::Skipped;
    }
    let tab = edge::tab_rect(side, monitor_work_area(&monitor), edge::along_center(side, card), scale);

    let logical = size.to_logical::<u32>(scale);
    if primary_mouse_down() {
        return ParkAttempt::Deferred;
    }
    // Relax Tao's minimum before enabling the native square constraint so
    // the minimum update does not shrink the window at its old origin.
    let _ = window.set_min_size(Some(tauri::LogicalSize::new(24.0, 24.0)));
    if !set_frame_constraint(window, tab.width) {
        let _ = window.set_min_size(Some(tauri::LogicalSize::new(320.0, 210.0)));
        return ParkAttempt::Skipped;
    }
    let parked = edge::Parked { side, tab, width: logical.width, height: logical.height };
    state.set(Some(parked));
    if !set_edge_rect(window, tab) {
        eprintln!("window: could not position folded icon");
    }
    if !app.state::<SettingsState>().snapshot().size_locked {
        let _ = window.set_resizable(false);
    }
    // State drives the button even if the OS did not apply the requested size.
    let _ = app.emit("edge-state", EdgeSnapshot::tab(parked, state.revision()));
    shape_window(app, window);
    ParkAttempt::Parked
}

/// Unfold the card back to the size and place it had before parking.
fn expand_from_edge(app: &AppHandle, window: &tauri::WebviewWindow) {
    let state = app.state::<edge::EdgeState>();
    let Some(parked) = state.parked() else {
        return;
    };
    let opened_under_pointer = window_rect(window).is_some_and(|rect| !cursor_outside(window, rect));
    let Some(monitor) = parked_monitor(window, parked.tab) else { return };
    let rect = edge::expanded_rect(parked, monitor_work_area(&monitor), monitor.scale_factor());

    if !set_frame_constraint(window, 0) {
        return;
    }
    if !set_edge_rect(window, rect) {
        // Keep the icon and its recovery actions live if the native resize failed.
        set_frame_constraint(window, parked.tab.width);
        set_edge_rect(window, parked.tab);
        return;
    }
    state.set(None);
    state.suppress_for(std::time::Duration::from_millis(1200));
    // Raising Tao's minimum while the window is still a tab resizes it
    // immediately at the old origin. Apply the final rectangle first.
    let _ = window.set_min_size(Some(tauri::LogicalSize::new(320.0, 210.0)));
    if !app.state::<SettingsState>().snapshot().size_locked {
        let _ = window.set_resizable(true);
    }
    shape_window(app, window);
    let _ = app.emit("edge-state", EdgeSnapshot::card());
    watch_preview_leave(app, window, opened_under_pointer);
}

fn monitor_work_area(monitor: &tauri::Monitor) -> edge::Rect {
    let work = monitor.work_area();
    edge::Rect {
        x: work.position.x,
        y: work.position.y,
        width: work.size.width as i32,
        height: work.size.height as i32,
    }
}

fn parked_monitor(window: &tauri::WebviewWindow, tab: edge::Rect) -> Option<tauri::Monitor> {
    window.monitor_from_point(f64::from(tab.x + tab.width / 2), f64::from(tab.y + tab.height / 2))
        .ok().flatten()
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten())
}

#[tauri::command]
fn get_edge_state(app: AppHandle) -> EdgeSnapshot {
    let state = app.state::<edge::EdgeState>();
    match state.parked() {
        Some(parked) => EdgeSnapshot::tab(parked, state.revision()),
        None => EdgeSnapshot::card(),
    }
}

#[tauri::command]
fn move_edge_tab(app: AppHandle, revision: u64, center: i32, delta: f64) {
    let state = app.state::<edge::EdgeState>();
    if state.revision() != revision || !delta.is_finite() {
        return;
    }
    let (Some(parked), Some(window)) = (state.parked(), app.get_webview_window("main")) else { return };
    let Some(monitor) = parked_monitor(&window, parked.tab) else { return };
    let scale = monitor.scale_factor();
    let center = center.saturating_add((delta * scale).round() as i32);
    let tab = edge::tab_rect(parked.side, monitor_work_area(&monitor), center, scale);
    if tab != parked.tab && set_frame_constraint(&window, tab.width) {
        if set_edge_rect(&window, tab) {
            state.update_tab(tab);
            shape_window(&app, &window);
        } else {
            set_frame_constraint(&window, parked.tab.width);
        }
    }
}

fn cursor_outside(window: &tauri::WebviewWindow, card: edge::Rect) -> bool {
    window.cursor_position().is_ok_and(|cursor| {
        cursor.x < f64::from(card.x) || cursor.x >= f64::from(card.right())
            || cursor.y < f64::from(card.y) || cursor.y >= f64::from(card.bottom())
    })
}

fn window_rect(window: &tauri::WebviewWindow) -> Option<edge::Rect> {
    let pos = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    Some(edge::Rect { x: pos.x, y: pos.y, width: size.width as i32, height: size.height as i32 })
}

/// A viewport resize can swallow WebView pointerleave. Track the pointer
/// until this preview closes or the card moves away from the edge.
fn watch_preview_leave(app: &AppHandle, window: &tauri::WebviewWindow, opened_under_pointer: bool) {
    let app = app.clone();
    let window = window.clone();
    let revision = app.state::<edge::EdgeState>().revision();
    std::thread::spawn(move || {
        let state = app.state::<edge::EdgeState>();
        let mut entered = opened_under_pointer;
        let mut outside_since = None;
        while state.revision() == revision && state.is_edge_preview() {
            std::thread::sleep(std::time::Duration::from_millis(40));
            let Some(rect) = window_rect(&window) else { break; };
            if !cursor_outside(&window, rect) {
                entered = true;
                outside_since = None;
            } else if primary_mouse_down() {
                outside_since = None;
            } else if entered {
                let start = outside_since.get_or_insert_with(std::time::Instant::now);
                if start.elapsed() >= std::time::Duration::from_millis(120) {
                    outside_since = None;
                    let handle = app.clone();
                    let candidate = window.clone();
                    let (finished, attempt) = std::sync::mpsc::sync_channel(1);
                    if app.run_on_main_thread(move || {
                        if handle.state::<edge::EdgeState>().revision() == revision {
                            park_if_near_edge(&handle, &candidate, true);
                        }
                        let _ = finished.send(());
                    }).is_err() || attempt.recv().is_err() {
                        break;
                    }
                    // Keep the preview armed. The button or pointer may have
                    // moved before the UI thread checked the latest geometry.
                }
            }
        }
    });
}

#[tauri::command]
fn expand_edge(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        expand_from_edge(&app, &window);
    }
}

/// Smallest and largest card the layout is built for.
///
/// These are set after the window exists, in logical units. Putting them in
/// `tauri.conf.json` is not equivalent: the config values are applied by the
/// runtime before the window is created and, at 200% scaling, clamped the
/// 760×440 default down to the minimum instead of leaving it alone.
fn clamp_size(window: &tauri::WebviewWindow) {
    let min = tauri::LogicalSize::new(320.0, 210.0);
    let max = tauri::LogicalSize::new(1100.0, 700.0);
    if let Err(error) = window.set_min_size(Some(min)) {
        eprintln!("window: cannot set minimum size ({error})");
    }
    if let Err(error) = window.set_max_size(Some(max)) {
        eprintln!("window: cannot set maximum size ({error})");
    }
}

/// Put the card back where the user left it, if that spot still exists.
///
/// A monitor may have been unplugged since the position was saved, so the point
/// is checked against the monitors that are present now; anything else falls
/// through to the centring the config asks for.
fn restore_geometry(
    window: &tauri::WebviewWindow,
    saved_position: Option<settings::WindowPosition>,
    saved_size: Option<settings::WindowSize>,
) {
    if let Some(saved) = saved_size {
        // Clamp to what the user can reach by dragging, so an edited or stale
        // file cannot produce a card that is invisible or larger than the screen.
        let width = saved.width.clamp(320, 1100) as f64;
        let height = saved.height.clamp(210, 700) as f64;
        if let Err(error) = window.set_size(tauri::LogicalSize::new(width, height)) {
            eprintln!("window: cannot restore size ({error})");
        }
    }

    let Some(saved) = saved_position else { return };
    let target = tauri::PhysicalPosition::new(saved.x, saved.y);

    if let Ok(monitors) = window.available_monitors() {
        let visible = monitors.iter().any(|monitor| {
            let origin = monitor.position();
            let size = monitor.size();
            target.x >= origin.x
                && target.y >= origin.y
                && target.x < origin.x + size.width as i32
                && target.y < origin.y + size.height as i32
        });
        if !visible {
            eprintln!("window: saved position is off-screen, ignoring it");
            return;
        }
    }

    if let Err(error) = window.set_position(target) {
        eprintln!("window: cannot restore position ({error})");
    }
}

#[cfg(target_os = "windows")]
fn primary_mouse_down() -> bool {
    #[link(name = "user32")]
    extern "system" { fn GetAsyncKeyState(key: i32) -> i16; }
    // VK_LBUTTON; the high bit reports the current state.
    unsafe { GetAsyncKeyState(1) < 0 }
}

#[cfg(not(target_os = "windows"))]
fn primary_mouse_down() -> bool { false }

/// Persist where the card sits and how big it is once the user stops dragging.
///
/// Moves and resizes arrive in bursts; writing on every one of them would hammer
/// the disk for no benefit, so the write is debounced. Both values are read back
/// from the window inside the delayed task, which keeps a move and a resize that
/// happen together from overwriting each other with stale halves.
fn watch_geometry(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    use std::sync::Arc;

    let pending = Arc::new(Mutex::new(edge::GeometryDebounce::default()));

    let app = app.clone();
    let watched = window.clone();
    window.on_window_event(move |event| {
        let moved = matches!(event, tauri::WindowEvent::Moved(_));
        let resized = matches!(event, tauri::WindowEvent::Resized(_));
        if !moved && !resized {
            return;
        }
        // A region is in device pixels, so it has to be rebuilt for the new size
        // (and for the new DPI when the window lands on another monitor).
        if resized {
            shape_window(&app, &watched);
        }
        // Parking moves and resizes the tab itself. Those events do not
        // represent a new card drag or geometry worth persisting.
        if app.state::<edge::EdgeState>().is_parked() {
            return;
        }
        // Expansion itself moves the window. Ignore that move during the
        // settling period, but retain movements the user makes while held.
        let user_moved = moved && (!app.state::<edge::EdgeState>().is_suppressed() || primary_mouse_down());
        let generation = pending.lock().unwrap_or_else(|e| e.into_inner()).record(user_moved);
        let pending = Arc::clone(&pending);
        let app = app.clone();
        let window = watched.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(600));
            loop {
                // Wait off the UI thread. A newer event owns the next check.
                while primary_mouse_down() {
                    if !pending.lock().unwrap_or_else(|e| e.into_inner()).is_current(generation) {
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                let (reply, result) = std::sync::mpsc::sync_channel(1);
                let handle = app.clone();
                let window = window.clone();
                let pending_ui = Arc::clone(&pending);
                if app.run_on_main_thread(move || {
                    let retry = settle_geometry(&handle, &window, &pending_ui, generation);
                    let _ = reply.send(retry);
                }).is_err() || result.recv() != Ok(true) {
                    break;
                }
            }
        });
    });
}


/// Process the current generation on the UI thread. Return true only when
/// a new press deferred parking and this worker still owns the movement.
fn settle_geometry(
    app: &AppHandle,
    window: &tauri::WebviewWindow,
    pending: &Mutex<edge::GeometryDebounce>,
    generation: u64,
) -> bool {
    let Some(moved) = pending.lock().unwrap_or_else(|e| e.into_inner()).movement(generation) else {
        return false;
    };
    let parked = app.state::<edge::EdgeState>().is_parked();
    let attempt = if moved && !parked {
        park_if_near_edge(app, window, false)
    } else {
        ParkAttempt::Skipped
    };
    let mut pending = pending.lock().unwrap_or_else(|e| e.into_inner());
    if !pending.is_current(generation) {
        return false;
    }
    let retry = pending.finish(generation, attempt == ParkAttempt::Deferred);
    drop(pending);
    if retry || parked || attempt == ParkAttempt::Parked {
        return retry;
    }

    let (Ok(scale), Ok(position), Ok(size)) =
        (window.scale_factor(), window.outer_position(), window.outer_size())
    else {
        return false;
    };
    let logical = size.to_logical::<u32>(scale);
    // Never persist the tab or a minimised window as the restored size.
    if logical.width < 320 || logical.height < 210 {
        return false;
    }
    let Some(state) = app.try_state::<SettingsState>() else {
        return false;
    };
    let updated = state.update(|settings| {
        settings.window_position = Some(settings::WindowPosition { x: position.x, y: position.y });
        settings.window_size = Some(settings::WindowSize { width: logical.width, height: logical.height });
    });
    if let Err(error) = settings::save(app, &updated) {
        eprintln!("settings: cannot persist window geometry ({error})");
    }
    false
}


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_card(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None::<Vec<&'static str>>,
        ))
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_pinned,
            set_size_locked,
            set_theme,
            get_autostart,
            set_autostart,
            get_ip_info,
            get_detection_mode,
            set_detection_mode,
            get_custom_proxy,
            validate_custom_proxy,
            save_custom_proxy,
            get_edge_state,
            move_edge_tab,
            expand_edge
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let stored = settings::load(&handle);
            app.manage(SettingsState::new(stored.clone()));
            app.manage(DetectionState::default());
            app.manage(edge::EdgeState::default());

            if let Some(window) = app.get_webview_window("main") {
                if let Err(error) = window.set_always_on_top(stored.pinned) {
                    eprintln!("window: cannot restore pin state ({error})");
                }
            }

            if let Err(error) = build_tray(&handle, stored.pinned) {
                // Without a tray, keep the taskbar as a recovery path.
                eprintln!("tray: unavailable ({error})");
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.set_skip_taskbar(false);
                }
            }

            // The window starts hidden so its first visible frame already has the
            // right pin state instead of floating up after a beat. The material is
            // applied on the same path: showing first would flash an opaque card.
            if let Some(window) = app.get_webview_window("main") {
                if COMPOSITOR_MATERIAL {
                    apply_material(&window);
                }
                clamp_size(&window);
                restore_geometry(&window, stored.window_position, stored.window_size);
                set_frame_constraint(&window, 0);
                if stored.size_locked {
                    if let Err(error) = window.set_resizable(false) {
                        eprintln!("window: cannot restore size lock ({error})");
                        app.state::<SettingsState>().update(|s| s.size_locked = false);
                    }
                }
                let _ = window.show();
                shape_window(&handle, &window);
                watch_geometry(&handle, &window);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running IP Monitor");
}
