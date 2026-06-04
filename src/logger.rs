use chrono::Local;
use std::{
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

const MAX_LOG_LINES: usize = 500;

fn log_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("zipline-upload/errors.log")
}

pub fn log_failure(file: &Path, err: &dyn std::fmt::Display) {
    let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("?");
    let line = format!("[{}] FAIL {} — {}", ts, name, err);

    eprintln!("{}", line);

    let path = log_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }

    // Read existing lines, append new one, then trim to MAX_LOG_LINES
    let existing: Vec<String> = if path.exists() {
        fs::File::open(&path)
            .map(|f| BufReader::new(f).lines().filter_map(|l| l.ok()).collect())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut lines = existing;
    lines.push(line);

    // Keep only the most recent MAX_LOG_LINES
    let start = lines.len().saturating_sub(MAX_LOG_LINES);
    let lines = &lines[start..];

    if let Ok(mut f) = OpenOptions::new().write(true).create(true).truncate(true).open(&path) {
        for l in lines {
            writeln!(f, "{}", l).ok();
        }
    }
}
