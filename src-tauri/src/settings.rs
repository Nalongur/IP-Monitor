//! Persisted window preferences (pin state, accent theme).
//!
//! These live in Rust rather than the webview's `localStorage` because the pin
//! state has to be applied *before* the window is shown: restoring it from the
//! frontend would flash the card at the wrong depth for a frame, which reads as
//! a bug on a desktop that is already crowded.
//!
//! Every failure path degrades to defaults. A corrupt or unreadable settings
//! file must never keep the card off the desktop — the same rule the spec sets
//! for the acrylic material fallback (§6.3).

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

const FILE_NAME: &str = "settings.json";

/// Accent palette. The variant name doubles as the `data-theme` attribute.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Lime,
    Aqua,
    Violet,
}

/// Wire shape shared with the frontend (camelCase via serde).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub pinned: bool,
    pub size_locked: bool,
    pub theme: Theme,
    /// Non-secret half of the custom proxy (spec §6.3). The password is never
    /// written here — it only lives in memory for the session (spec §10.2).
    pub custom_proxy: Option<CustomProxySettings>,
    /// Last window position in physical pixels. A widget that forgets where it
    /// was parked is worse than no persistence at all.
    pub window_position: Option<WindowPosition>,
    /// Last window size in logical pixels, so the card reopens at the size the
    /// user chose instead of snapping back to the default every launch.
    pub window_size: Option<WindowSize>,
}

/// Physical top-left corner of the card.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
}

/// Logical size of the card. Logical, not physical: the same settings file may be
/// read on a display with a different scale factor.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            pinned: true,
            size_locked: false,
            theme: Theme::Lime,
            custom_proxy: None,
            window_position: None,
            window_size: None,
        }
    }
}

/// Everything about a custom proxy that is safe to keep on disk.
///
/// The password is not here in clear text: `password_blob` holds a DPAPI blob
/// that only this Windows account can open (spec §10.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomProxySettings {
    pub protocol: crate::proxy::ProxyProtocol,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password_blob: Option<String>,
}

/// Managed state. Keeps the lock (and its poisoning story) out of the commands.
pub struct SettingsState(Mutex<Settings>);

impl SettingsState {
    pub fn new(settings: Settings) -> Self {
        Self(Mutex::new(settings))
    }

    /// A poisoned lock still carries a usable value; recover it rather than panic.
    fn guard(&self) -> std::sync::MutexGuard<'_, Settings> {
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn pinned(&self) -> bool {
        self.guard().pinned
    }

    pub fn snapshot(&self) -> Settings {
        self.guard().clone()
    }

    pub fn update(&self, change: impl FnOnce(&mut Settings)) -> Settings {
        let mut guard = self.guard();
        change(&mut guard);
        guard.clone()
    }
}

fn path<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join(FILE_NAME))
}

/// Missing or malformed files are not an error condition: start from defaults.
pub fn load<R: Runtime>(app: &AppHandle<R>) -> Settings {
    let Some(path) = path(app) else {
        eprintln!("settings: no config directory, starting from defaults");
        return Settings::default();
    };
    let Ok(raw) = fs::read_to_string(&path) else {
        return Settings::default();
    };
    serde_json::from_str(&raw).unwrap_or_else(|error| {
        eprintln!(
            "settings: {} is unreadable ({error}), starting from defaults",
            path.display()
        );
        Settings::default()
    })
}

/// Write to a sibling file first, then rename over the target.
///
/// A crash halfway through the write then leaves the previous settings intact
/// instead of a truncated file that would silently reset the pin state.
pub fn save<R: Runtime>(app: &AppHandle<R>, settings: &Settings) -> io::Result<()> {
    let path = path(app).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "no application config directory")
    })?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_vec_pretty(settings).map_err(io::Error::other)?;
    let staging = path.with_extension("json.tmp");
    fs::write(&staging, payload)?;
    fs::rename(&staging, &path)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_settings_default_to_unlocked_and_lock_round_trips() {
        let mut settings: Settings = serde_json::from_str(r#"{"pinned":false}"#).unwrap();
        assert!(!settings.size_locked);
        assert!(!settings.pinned);
        settings.size_locked = true;
        let saved = serde_json::to_string(&settings).unwrap();
        let restored: Settings = serde_json::from_str(&saved).unwrap();
        assert!(restored.size_locked);
    }
}
