mod config;
mod install;
mod upload;
mod notify;
mod logger;
mod gui;

use std::{env, path::PathBuf, process};

pub struct AppFlags {
    pub kde: Option<bool>,
}

impl AppFlags {
    pub fn is_kde(&self) -> bool {
        self.kde.unwrap_or_else(detect_kde)
    }
}

fn detect_kde() -> bool {
    env::var("KDE_FULL_SESSION").is_ok()
        || env::var("DESKTOP_SESSION")
            .unwrap_or_default()
            .to_lowercase()
            .contains("kde")
        || env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_lowercase()
            .contains("kde")
}

fn main() {
    let raw_args: Vec<String> = env::args().collect();

    let mut file_arg: Option<PathBuf> = None;
    let mut kde_override: Option<bool> = None;

    let mut i = 1;
    while i < raw_args.len() {
        match raw_args[i].as_str() {
            "--kde"         => kde_override = Some(true),
            "--no-kde"      => kde_override = Some(false),
            "--help" | "-h" => { print_help(); process::exit(0); }
            arg if !arg.starts_with("--") => file_arg = Some(PathBuf::from(arg)),
            unknown => {
                eprintln!("Unknown flag: {}", unknown);
                process::exit(1);
            }
        }
        i += 1;
    }

    let flags = AppFlags { kde: kde_override };

    install::ensure_installed();

    if let Some(file) = file_arg {
        if !file.exists() {
            eprintln!("File not found: {}", file.display());
            process::exit(1);
        }

        let cfg = config::load_or_setup();

        match upload::upload_file(&cfg, &file) {
            Ok(url) => {
                let copied = copy_to_clipboard(&url);
                notify::success(&file, &url, flags.is_kde(), copied);
            }
            Err(e) => {
                logger::log_failure(&file, &e);
                notify::failure(&file, &e);
                eprintln!("Upload failed: {}", e);
                process::exit(1);
            }
        }
    } else {
        gui::run(flags);
    }
}

/// Copy text to clipboard. Returns true if it succeeded.
/// On Wayland uses wl-copy (daemonises itself so content survives process exit).
/// On X11 tries xclip then xsel.
pub fn copy_to_clipboard(text: &str) -> bool {
    let is_wayland = env::var("WAYLAND_DISPLAY").is_ok();

    if is_wayland {
        try_copy_with("wl-copy", &[], text)
    } else {
        try_copy_with("xclip", &["-selection", "clipboard"], text)
            || try_copy_with("xsel", &["--clipboard", "--input"], text)
    }
}

fn try_copy_with(cmd: &str, args: &[&str], text: &str) -> bool {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = match Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    if let Some(stdin) = child.stdin.as_mut() {
        if stdin.write_all(text.as_bytes()).is_err() {
            return false;
        }
    }
    // wl-copy daemonises; don't wait so we don't block
    true
}

fn print_help() {
    println!(
        "zipline-upload — upload files to Zipline\n\
         \n\
         USAGE:\n\
             zipline-upload [FLAGS] [FILE]\n\
         \n\
         FLAGS:\n\
             --kde           Force KDE notification style\n\
             --no-kde        Force generic notification style\n\
             --help          Show this help\n\
         \n\
         Without FILE, opens the drag & drop GUI.\n\
         \n\
         To reconfigure, delete ~/.config/zipline-upload/config.sxcu\n\
         and re-run — setup will prompt for a new .sxcu file."
    );
}
