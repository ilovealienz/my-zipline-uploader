use crate::config::SxcuConfig;
use crate::settings::Settings;
use std::path::Path;

pub struct UploadError(pub String);

impl std::fmt::Display for UploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub fn upload_file(cfg: &SxcuConfig, settings: &Settings, file: &Path) -> Result<String, UploadError> {
    let auth = cfg
        .authorization()
        .ok_or_else(|| UploadError("No authorization header in config".into()))?
        .to_string();

    let file_name = file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("upload")
        .to_string();

    let mime = mime_guess::from_path(file)
        .first_or_octet_stream()
        .to_string();

    let bytes = std::fs::read(file)
        .map_err(|e| UploadError(format!("Could not read file: {}", e)))?;

    let part = reqwest::blocking::multipart::Part::bytes(bytes)
        .file_name(file_name)
        .mime_str(&mime)
        .map_err(|e| UploadError(e.to_string()))?;

    let form = reqwest::blocking::multipart::Form::new()
        .part(cfg.file_form_name.clone(), part);

    let client = reqwest::blocking::Client::new();

    // Start with the .sxcu headers (minus authorization which goes separately)
    let mut builder = client
        .post(&cfg.request_url)
        .header("authorization", auth)
        .multipart(form);

    // Apply .sxcu headers first (skip authorization, already set)
    for (k, v) in &cfg.headers {
        let key = k.to_lowercase();
        if key != "authorization" {
            builder = builder.header(k.clone(), v.clone());
        }
    }

    // Layer settings headers on top — only for keys not already in .sxcu
    let sxcu_keys: std::collections::HashSet<String> = cfg.headers
        .keys()
        .map(|k| k.to_lowercase())
        .collect();

    for (k, v) in settings.extra_headers() {
        if !sxcu_keys.contains(&k.to_lowercase()) {
            builder = builder.header(k, v);
        }
    }

    let response = builder
        .send()
        .map_err(|e| UploadError(format!("Request failed: {}", e)))?;

    let status = response.status();
    let body = response.text().unwrap_or_else(|_| "(no response body)".into());

    if !status.is_success() {
        return Err(UploadError(format!(
            "{} {}: {}",
            status.as_u16(),
            status.canonical_reason().unwrap_or(""),
            body
        )));
    }

    let json: serde_json::Value = serde_json::from_str(&body)
        .map_err(|_| UploadError(format!("Invalid JSON response: {}", body)))?;

    json["files"][0]["url"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| UploadError(format!("Could not find files[0].url in: {}", body)))
}
