use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

/// Id of the open progress notification, so later updates and the final
/// "Uploaded" / "Upload failed" replace it (freedesktop replaces_id, which
/// every notification server supports, unlike the synchronous hint).
static PROGRESS_ID: Mutex<Option<u32>> = Mutex::new(None);

/// Adds --replace-id for the open progress notification, if any.
fn replacing(cmd: &mut Command, keep: bool) {
    let mut id = PROGRESS_ID.lock().unwrap();
    if let Some(n) = *id {
        cmd.arg(format!("--replace-id={}", n));
    }
    if !keep {
        *id = None;
    }
}

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

/// Notifications with this hint replace each other (mako, dunst), so the
/// progress popup turns into the result instead of piling up.
const SYNC: &str = "string:x-canonical-private-synchronous:zipline-upload";

pub fn progress(name: &str, pct: u64, sent: u64, total: u64) {
    let mut cmd = Command::new("notify-send");
    cmd.arg("--app-name=Zipline Upload")
        .arg("--expire-time=0")
        .arg("--print-id")
        .arg(format!("--hint={}", SYNC))
        .arg(format!("--hint=int:value:{}", pct))
        .arg(format!("Uploading — {}", name))
        .arg(format!(
            "{}% · {} of {}",
            pct,
            crate::upload::human(sent),
            crate::upload::human(total)
        ));
    replacing(&mut cmd, true);
    if let Ok(out) = cmd.output() {
        if let Ok(n) = String::from_utf8_lossy(&out.stdout).trim().parse::<u32>() {
            *PROGRESS_ID.lock().unwrap() = Some(n);
        }
    }
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
       .arg(format!("--hint={}", SYNC))
       .arg(format!("Uploaded — {}", name))
       .arg(&body);

    if is_image(file) {
        if let Some(p) = file.to_str() {
            cmd.arg(format!("--icon={}", p));
        }
    }

    replacing(&mut cmd, false);
    cmd.spawn().ok();
}

pub fn failure(file: &Path, err: &dyn std::fmt::Display) {
    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("file");

    let mut cmd = Command::new("notify-send");
    cmd.arg("--app-name=Zipline Upload")
        .arg("--expire-time=8000")
        .arg(format!("--hint={}", SYNC))
        .arg("--urgency=critical")
        .arg(format!("Upload failed — {}", name))
        .arg(err.to_string());
    replacing(&mut cmd, false);
    cmd.spawn().ok();
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
