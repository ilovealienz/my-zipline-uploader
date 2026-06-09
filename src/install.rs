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

const DESKTOP_ADVANCED_TEMPLATE: &str = "[Desktop Entry]
Name=Zipline Upload (Advanced)
Comment=Upload a file to Zipline with custom options
Exec={bin_path} --advanced %f
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

fn desktop_advanced_dest() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local/share/applications/zipline-upload-advanced.desktop")
}

fn marker_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("zipline-upload/installed")
}

pub fn ensure_installed() {
    let current = env::current_exe().expect("Cannot determine current executable path");
    let marker = marker_path();
    let needs_full_install = !marker.exists();
    let needs_advanced = !desktop_advanced_dest().exists();

    if !needs_full_install && !needs_advanced {
        return;
    }

    if needs_full_install {
        println!("First run — registering zipline-upload...");
    } else if needs_advanced {
        // Upgrading from v0.2 — just add the advanced entry
        println!("Upgrading — adding Zipline Upload (Advanced) entry...");
    }

    let apps_dir = desktop_dest().parent().unwrap().to_path_buf();
    fs::create_dir_all(&apps_dir)
        .unwrap_or_else(|e| eprintln!("Could not create applications dir: {}", e));

    let bin = current.display().to_string();

    // Write main .desktop (only on full install)
    if needs_full_install {
        let content = DESKTOP_TEMPLATE.replace("{bin_path}", &bin);
        write_desktop(&desktop_dest(), &content);
    }

    // Write advanced .desktop (on full install or upgrade)
    let content = DESKTOP_ADVANCED_TEMPLATE.replace("{bin_path}", &bin);
    write_desktop(&desktop_advanced_dest(), &content);

    // Update desktop database
    run_update_db(&apps_dir);

    if needs_full_install {
        // Write marker
        let marker = marker_path();
        fs::create_dir_all(marker.parent().unwrap()).ok();
        fs::write(&marker, bin).ok();

        println!("Registered → {}", current.display());
        println!();
        println!("Done. You can now:");
        println!("  CLI:      zipline-upload /path/to/file");
        println!("  GUI:      zipline-upload");
        println!("  Advanced: zipline-upload --advanced /path/to/file");
        println!("  Right-click any file → Open With → Zipline Upload");
    } else {
        println!("Done. Right-click any file → Open With → Zipline Upload (Advanced)");
    }
}

pub fn uninstall() {
    println!("Uninstalling zipline-upload...");

    let mut any = false;

    for path in &[desktop_dest(), desktop_advanced_dest()] {
        if path.exists() {
            if let Err(e) = fs::remove_file(path) {
                eprintln!("Could not remove {}: {}", path.display(), e);
            } else {
                println!("Removed {}", path.display());
                any = true;
            }
        }
    }

    let marker = marker_path();
    if marker.exists() {
        fs::remove_file(&marker).ok();
        println!("Removed marker file");
        any = true;
    }

    if any {
        let apps_dir = desktop_dest().parent().unwrap().to_path_buf();
        run_update_db(&apps_dir);
        println!();
        println!("Uninstalled. You can now delete the binary.");
        println!("Your config and settings in ~/.config/zipline-upload/ have been kept.");
    } else {
        println!("Nothing to uninstall.");
    }
}

fn write_desktop(path: &PathBuf, content: &str) {
    if let Err(e) = fs::write(path, content) {
        eprintln!("Could not write {}: {}", path.display(), e);
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(path, perms).ok();
        }
    }
}

fn run_update_db(apps_dir: &PathBuf) {
    match Command::new("update-desktop-database").arg(apps_dir).status() {
        Ok(s) if s.success() => println!("Desktop database updated."),
        _ => eprintln!("Could not run update-desktop-database — run it manually if needed."),
    }
}
