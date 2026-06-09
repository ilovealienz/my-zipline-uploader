use crate::config::SxcuConfig;

pub struct ShortenError(pub String);

impl std::fmt::Display for ShortenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub fn shorten_url(cfg: &SxcuConfig, url: &str, vanity: Option<&str>) -> Result<String, ShortenError> {
    let auth = cfg
        .authorization()
        .ok_or_else(|| ShortenError("No authorization header in config".into()))?
        .to_string();

    let endpoint = derive_shorten_endpoint(&cfg.request_url)
        .ok_or_else(|| ShortenError(format!("Could not derive shorten endpoint from: {}", cfg.request_url)))?;

    // v4 API uses "destination" field
    let mut body = serde_json::json!({ "destination": url });
    if let Some(v) = vanity {
        if !v.is_empty() {
            body["vanity"] = serde_json::Value::String(v.to_string());
        }
    }

    let client = reqwest::blocking::Client::new();
    let response = client
        .post(&endpoint)
        .header("authorization", auth)
        .header("content-type", "application/json")
        .body(body.to_string())
        .send()
        .map_err(|e| ShortenError(format!("Request failed: {}", e)))?;

    let status = response.status();
    let body = response.text().unwrap_or_else(|_| "(no response body)".into());

    if !status.is_success() {
        return Err(ShortenError(format!(
            "{} {}: {}",
            status.as_u16(),
            status.canonical_reason().unwrap_or(""),
            body
        )));
    }

    let json: serde_json::Value = serde_json::from_str(&body)
        .map_err(|_| ShortenError(format!("Invalid JSON response: {}", body)))?;

    // v4 returns the url in the "url" field
    json["url"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| ShortenError(format!("Could not find url in response: {}", body)))
}

fn derive_shorten_endpoint(upload_url: &str) -> Option<String> {
    // v4: /api/upload -> /api/user/urls
    for suffix in &["/api/upload", "/api/upload/"] {
        if let Some(base) = upload_url.strip_suffix(suffix) {
            return Some(format!("{}/api/user/urls", base));
        }
    }
    // Fallback: replace path at third slash
    let mut slashes = 0;
    for (i, c) in upload_url.char_indices() {
        if c == '/' { slashes += 1; }
        if slashes == 3 {
            return Some(format!("{}/api/user/urls", &upload_url[..i]));
        }
    }
    None
}
