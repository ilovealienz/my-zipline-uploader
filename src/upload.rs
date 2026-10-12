use crate::config::SxcuConfig;
use crate::notify;
use crate::settings::Settings;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::time::{Duration, Instant};

pub struct UploadError(pub String);

impl std::fmt::Display for UploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Files at least this big get a progress notification while uploading.
const PROGRESS_FROM: u64 = 20 * 1024 * 1024;

/// Wraps the file as it is read for sending, so the upload streams from
/// disk in small pieces (the whole file is never in memory) and can
/// report how far along it is.
struct Progress<R> {
    inner: R,
    sent: u64,
    total: u64,
    on_progress: Box<dyn FnMut(u64, u64) + Send>,
}

impl<R: Read> Read for Progress<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.sent += n as u64;
        (self.on_progress)(self.sent, self.total);
        Ok(n)
    }
}

/// Upload with the headers from your saved settings (the CLI path).
pub fn upload_file(cfg: &SxcuConfig, settings: &Settings, file: &Path) -> Result<String, UploadError> {
    upload_with_headers(cfg, file, settings.extra_headers())
}

/// The actual HTTP upload: .sxcu headers first, extra headers layered on
/// top for any key the .sxcu doesn't already set.
pub fn upload_with_headers(
    cfg: &SxcuConfig,
    file: &Path,
    extra: Vec<(String, String)>,
) -> Result<String, UploadError> {
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

    let handle = File::open(file).map_err(|e| UploadError(format!("Could not read file: {}", e)))?;
    let total = handle
        .metadata()
        .map_err(|e| UploadError(format!("Could not read file: {}", e)))?
        .len();

    // progress notification for big files, at most every 10% and 1s apart
    let progress_name = file_name.clone();
    let mut last_shown = 0u64;
    let mut last_time = Instant::now();
    let reader = Progress {
        inner: handle,
        sent: 0,
        total,
        on_progress: Box::new(move |sent, total| {
            if total < PROGRESS_FROM || sent >= total {
                return;
            }
            let pct = sent * 100 / total;
            if pct >= last_shown + 10 && last_time.elapsed() >= Duration::from_secs(1) {
                last_shown = pct - pct % 10;
                last_time = Instant::now();
                notify::progress(&progress_name, pct, sent, total);
            }
        }),
    };

    let part = reqwest::blocking::multipart::Part::reader_with_length(reader, total)
        .file_name(file_name)
        .mime_str(&mime)
        .map_err(|e| UploadError(e.to_string()))?;

    let form = reqwest::blocking::multipart::Form::new()
        .part(cfg.file_form_name.clone(), part);

    // reqwest's blocking client gives a whole request 30 seconds by
    // default, which a big upload can't finish in; only limit connecting
    let client = reqwest::blocking::Client::builder()
        .timeout(None)
        .connect_timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| UploadError(e.to_string()))?;

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

    // Layer extra headers on top — only for keys not already in .sxcu
    let sxcu_keys: std::collections::HashSet<String> = cfg.headers
        .keys()
        .map(|k| k.to_lowercase())
        .collect();

    for (k, v) in extra {
        if !sxcu_keys.contains(&k.to_lowercase()) {
            builder = builder.header(k, v);
        }
    }

    let response = builder
        .send()
        .map_err(|e| UploadError(format!("Request failed: {}", e)))?;

    let status = response.status();
    let body = response.text().unwrap_or_else(|_| "(no response body)".into());

    if status.as_u16() == 413 {
        return Err(UploadError(format!(
            "The file ({}) is bigger than the server accepts. Raise Zipline's max file size, or nginx's client_max_body_size.",
            human(total)
        )));
    }

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

/// 1536 -> "1.5 KB", 734003200 -> "700.0 MB"
pub fn human(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < units.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 { format!("{} B", bytes) } else { format!("{:.1} {}", v, units[i]) }
}
