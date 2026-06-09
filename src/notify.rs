use std::path::Path;
use std::process::Command;

fn is_image(file: &Path) -> bool {
    matches!(
        file.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some(
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif"
            | "bmp" | "tiff" | "tif" | "ico" | "heic" | "heif"
        )
    )
}

pub fn success(file: &Path, url: &str, kde: bool, copied: bool) {
    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let clipboard_line = if copied { "\nCopied to clipboard" } else { "" };

    let body = if kde {
        format!("<a href=\"{url}\">{url}</a>{clipboard_line}")
    } else {
        format!("{url}{clipboard_line}")
    };

    let mut cmd = Command::new("notify-send");
    cmd.arg("--app-name=Zipline Upload")
       .arg("--expire-time=5000")
       .arg(format!("Uploaded — {}", name))
       .arg(&body);

    if is_image(file) {
        if let Some(p) = file.to_str() {
            cmd.arg(format!("--icon={}", p));
        }
    }

    cmd.spawn().ok();
}

pub fn failure(file: &Path, err: &dyn std::fmt::Display) {
    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("file");

    Command::new("notify-send")
        .arg("--app-name=Zipline Upload")
        .arg("--expire-time=8000")
        .arg("--urgency=critical")
        .arg(format!("Upload failed — {}", name))
        .arg(err.to_string())
        .spawn()
        .ok();
}

pub fn blocked(file: &Path, reason: &str) {
    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("file");

    Command::new("notify-send")
        .arg("--app-name=Zipline Upload")
        .arg("--expire-time=6000")
        .arg("--urgency=normal")
        .arg(format!("Blocked — {}", name))
        .arg(reason)
        .spawn()
        .ok();
}

pub fn shorten_success(url: &str, kde: bool, copied: bool) {
    let clipboard_line = if copied { "\nCopied to clipboard" } else { "" };
    let body = if kde {
        format!("<a href=\"{url}\">{url}</a>{clipboard_line}")
    } else {
        format!("{url}{clipboard_line}")
    };

    Command::new("notify-send")
        .arg("--app-name=Zipline Upload")
        .arg("--expire-time=5000")
        .arg("Shortened URL")
        .arg(&body)
        .spawn()
        .ok();
}

pub fn shorten_failure(err: &dyn std::fmt::Display) {
    Command::new("notify-send")
        .arg("--app-name=Zipline Upload")
        .arg("--expire-time=8000")
        .arg("--urgency=critical")
        .arg("Shorten failed")
        .arg(err.to_string())
        .spawn()
        .ok();
}
