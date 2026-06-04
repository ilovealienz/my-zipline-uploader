use std::{env, fs, path::PathBuf, process::Command};

const DESKTOP_TEMPLATE: &str = "[Desktop Entry]
Name=Zipline Upload
Comment=Upload a file to Zipline and copy the URL to clipboard
Exec={bin_path} %f
Icon=document-send
Terminal=false
Type=Application
MimeType=application/octet-stream;
NoDisplay=false
Categories=Utility;
";

fn desktop_dest() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local/share/applications/zipline-upload.desktop")
}

fn marker_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("zipline-upload/installed")
}

pub fn ensure_installed() {
    if marker_path().exists() {
        return;
    }

    // Use the path of the currently running binary — no copying
    let current = env::current_exe().expect("Cannot determine current executable path");

    println!("First run — registering zipline-upload...");

    // Write .desktop pointing at wherever the binary lives right now
    let desktop = desktop_dest();
    fs::create_dir_all(desktop.parent().unwrap())
        .unwrap_or_else(|e| eprintln!("Could not create applications dir: {}", e));

    let content = DESKTOP_TEMPLATE.replace("{bin_path}", &current.display().to_string());
    if let Err(e) = fs::write(&desktop, &content) {
        eprintln!("Could not write .desktop file: {}", e);
        return;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(&desktop) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&desktop, perms).ok();
        }
    }

    // Update desktop database so it shows up in Open With immediately
    let apps_dir = desktop.parent().unwrap();
    match Command::new("update-desktop-database").arg(apps_dir).status() {
        Ok(s) if s.success() => {}
        _ => eprintln!("Could not run update-desktop-database — run it manually if needed."),
    }

    // Write marker file containing the registered binary path
    let marker = marker_path();
    fs::create_dir_all(marker.parent().unwrap()).ok();
    fs::write(&marker, current.display().to_string()).ok();

    println!("Registered → {}", current.display());
    println!("Open With entry created in Dolphin.");
    println!();
}
