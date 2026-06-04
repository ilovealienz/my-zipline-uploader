use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SxcuConfig {
    #[serde(rename = "RequestURL")]
    pub request_url: String,

    #[serde(rename = "FileFormName")]
    pub file_form_name: String,

    #[serde(rename = "Headers")]
    pub headers: HashMap<String, String>,

    #[serde(rename = "URL", default)]
    pub url_template: String,
}

impl SxcuConfig {
    pub fn authorization(&self) -> Option<&str> {
        self.headers
            .get("authorization")
            .map(String::as_str)
            .or_else(|| self.headers.get("Authorization").map(String::as_str))
    }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("zipline-upload/config.sxcu")
}

pub fn config_display_path() -> String {
    let p = config_path();
    if p.exists() {
        if let Some(home) = dirs::home_dir() {
            if let Ok(rel) = p.strip_prefix(&home) {
                return format!("~/{}", rel.display());
            }
        }
        p.display().to_string()
    } else {
        "(not set)".to_string()
    }
}

pub fn load_from_file(path: &Path) -> Result<SxcuConfig, Box<dyn std::error::Error>> {
    let text = fs::read_to_string(path)?;
    let cfg: SxcuConfig = serde_json::from_str(&text)?;
    Ok(cfg)
}

pub fn load_or_setup() -> SxcuConfig {
    let path = config_path();
    if path.exists() {
        return load_from_file(&path).unwrap_or_else(|e| {
            eprintln!("Config parse error ({}), re-running setup.", e);
            run_setup()
        });
    }
    run_setup()
}


fn run_setup() -> SxcuConfig {
    let sxcu_path = pick_sxcu_file();
    let cfg = load_from_file(&sxcu_path).unwrap_or_else(|e| {
        eprintln!("Could not parse .sxcu file: {}", e);
        std::process::exit(1);
    });
    save_sxcu(&sxcu_path);
    cfg
}

fn pick_sxcu_file() -> PathBuf {
    rfd::FileDialog::new()
        .set_title("Select your Zipline .sxcu config file")
        .add_filter("ShareX Config", &["sxcu"])
        .pick_file()
        .unwrap_or_else(|| {
            eprintln!("No file selected.");
            std::process::exit(1);
        })
}

pub fn save_sxcu(src: &Path) {
    let dest = config_path();
    fs::create_dir_all(dest.parent().unwrap())
        .unwrap_or_else(|e| eprintln!("Could not create config dir: {}", e));
    if let Err(e) = fs::copy(src, &dest) {
        eprintln!("Could not save config: {}", e);
    }
}
