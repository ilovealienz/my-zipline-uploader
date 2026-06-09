use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    // App
    pub kde_notifications: Option<bool>, // None = auto-detect

    // Upload defaults — None = don't send the header
    pub format: Option<String>,           // random/date/uuid/name/gfycat
    pub original_name: Option<bool>,
    pub deletes_at: Option<String>,       // e.g. "1h", "1d", blank = disabled
    pub max_views: Option<u32>,           // 0 = disabled
    pub password: Option<String>,
    pub folder_id: Option<String>,
    pub domain: Option<String>,
    pub image_compression_percent: Option<u32>, // 0 = disabled
    pub image_compression_type: Option<String>, // jpg/png/webp/jxl

    // Extension allowlist — empty = no check
    pub allowed_extensions: Option<Vec<String>>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            kde_notifications: None,
            format: None,
            original_name: None,
            deletes_at: None,
            max_views: None,
            password: None,
            folder_id: None,
            domain: None,
            image_compression_percent: None,
            image_compression_type: None,
            allowed_extensions: None,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let path = settings_path();
        if path.exists() {
            fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) {
        let path = settings_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).ok();
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            fs::write(&path, json).ok();
        }
    }

    /// Build extra headers from settings. Only includes headers that have
    /// been explicitly set — nothing is sent for default/unset values.
    pub fn extra_headers(&self) -> Vec<(String, String)> {
        let mut headers = Vec::new();

        if let Some(fmt) = &self.format {
            if !fmt.is_empty() {
                headers.push(("x-zipline-format".into(), fmt.clone()));
            }
        }
        if let Some(true) = self.original_name {
            headers.push(("x-zipline-original-name".into(), "true".into()));
        }
        if let Some(deletes_at) = &self.deletes_at {
            if !deletes_at.is_empty() {
                headers.push(("x-zipline-deletes-at".into(), deletes_at.clone()));
            }
        }
        if let Some(views) = self.max_views {
            if views > 0 {
                headers.push(("x-zipline-max-views".into(), views.to_string()));
            }
        }
        if let Some(pw) = &self.password {
            if !pw.is_empty() {
                headers.push(("x-zipline-password".into(), pw.clone()));
            }
        }
        if let Some(folder) = &self.folder_id {
            if !folder.is_empty() {
                headers.push(("x-zipline-folder".into(), folder.clone()));
            }
        }
        if let Some(domain) = &self.domain {
            if !domain.is_empty() {
                headers.push(("x-zipline-domain".into(), domain.clone()));
            }
        }
        if let Some(pct) = self.image_compression_percent {
            if pct > 0 {
                headers.push(("x-zipline-image-compression-percent".into(), pct.to_string()));
                if let Some(fmt) = &self.image_compression_type {
                    if !fmt.is_empty() {
                        headers.push(("x-zipline-image-compression-type".into(), fmt.clone()));
                    }
                }
            }
        }

        headers
    }

    /// Check if a file extension is allowed. Returns true if no allowlist is
    /// configured, or if the extension is in the list.
    pub fn extension_allowed(&self, file: &std::path::Path) -> bool {
        let exts = match &self.allowed_extensions {
            Some(e) if !e.is_empty() => e,
            _ => return true,
        };
        let ext = file
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        exts.iter().any(|e| e.to_lowercase() == ext)
    }
}

fn settings_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("zipline-upload/settings.json")
}
