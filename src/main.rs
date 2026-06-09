mod config;
mod install;
mod upload;
mod notify;
mod logger;
mod settings;
mod shorten;
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

pub fn detect_kde() -> bool {
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
    let mut advanced = false;
    let mut uninstall = false;

    let mut i = 1;
    while i < raw_args.len() {
        match raw_args[i].as_str() {
            "--kde"         => kde_override = Some(true),
            "--no-kde"      => kde_override = Some(false),
            "--advanced"    => advanced = true,
            "--uninstall"   => uninstall = true,
            "--help" | "-h" => { print_help(); process::exit(0); }
            arg if !arg.starts_with("--") => file_arg = Some(PathBuf::from(arg)),
            unknown => {
                eprintln!("Unknown flag: {}", unknown);
                process::exit(1);
            }
        }
        i += 1;
    }

    if uninstall {
        install::uninstall();
        process::exit(0);
    }

    let s = settings::Settings::load();
    let kde = if let Some(k) = kde_override {
        k
    } else if let Some(k) = s.kde_notifications {
        k
    } else {
        detect_kde()
    };

    let flags = AppFlags { kde: Some(kde) };

    // Handles first run, upgrades, and no-ops if already up to date
    install::ensure_installed();

    if let Some(file) = file_arg {
        if !file.exists() {
            eprintln!("File not found: {}", file.display());
            process::exit(1);
        }

        if advanced {
            // Open GUI with file pre-loaded and override panel open
            gui::run_with_file(flags, file);
        } else {
            // Silent CLI upload

            if !s.extension_allowed(&file) {
                let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("unknown");
                let msg = format!(".{} is not in your allowed extensions list", ext);
                notify::blocked(&file, &msg);
                eprintln!("{}", msg);
                process::exit(1);
            }

            let cfg = config::load_or_setup();

            match upload::upload_file(&cfg, &s, &file) {
                Ok(url) => {
                    let copied = copy_to_clipboard(&url);
                    notify::success(&file, &url, kde, copied);
                }
                Err(e) => {
                    logger::log_failure(&file, &e);
                    notify::failure(&file, &e);
                    eprintln!("Upload failed: {}", e);
                    process::exit(1);
                }
            }
        }
    } else {
        gui::run(flags);
    }
}

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
    let mut child = match Command::new(cmd).args(args).stdin(Stdio::piped()).spawn() {
        Ok(c) => c,
        Err(_) => return false,
    };
    if let Some(stdin) = child.stdin.as_mut() {
        if stdin.write_all(text.as_bytes()).is_err() {
            return false;
        }
    }
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
             --advanced      Open GUI with file pre-loaded and options panel open\n\
             --uninstall     Remove desktop entries and marker file\n\
             --kde           Force KDE notification style\n\
             --no-kde        Force generic notification style\n\
             --help          Show this help\n\
         \n\
         Without FILE, opens the drag & drop GUI."
    );
}
