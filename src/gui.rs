use crate::{config, copy_to_clipboard, logger, notify, settings::Settings, upload, shorten, AppFlags};
use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

pub fn run(flags: AppFlags) {
    let kde = flags.is_kde();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Zipline Upload")
            .with_inner_size([460.0, 420.0])
            .with_resizable(false)
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "Zipline Upload",
        options,
        Box::new(move |_cc| Ok(Box::new(App::new(kde)))),
    )
    .unwrap_or_else(|e| eprintln!("GUI error: {}", e));
}

enum UploadResult {
    Ok { url: String, copied: bool },
    Err(String),
}

enum Status {
    Idle,
    Uploading,
    Done { url: String, copied: bool },
    Error(String),
    Blocked(String),
}

#[derive(Clone)]
struct UploadOptions {
    format: String,
    original_name: bool,
    deletes_at: String,
    deletes_at_preset: usize, // 0=never 1=1h 2=3h 3=1d 4=1w 5=custom
    custom_amount: String,
    custom_unit: usize, // 0=minutes 1=hours 2=days 3=weeks 4=years
    max_views: String,
    password: String,
    folder_id: String,
    domain: String,
    image_compression_percent: String,
    image_compression_type: String,
}

const EXPIRY_PRESETS: &[&str] = &["Never", "1h", "3h", "1d", "1w", "Custom"];
const EXPIRY_UNITS: &[(&str, &str)] = &[("minutes", "m"), ("hours", "h"), ("days", "d"), ("weeks", "w"), ("years", "y")];
const FORMATS: &[&str] = &["(default)", "random", "date", "uuid", "name", "gfycat"];
const COMPRESSION_TYPES: &[&str] = &["jpg", "png", "webp", "jxl"];

impl UploadOptions {
    fn from_settings(s: &Settings) -> Self {
        let (deletes_at, preset) = match s.deletes_at.as_deref() {
            None | Some("") => (String::new(), 0),
            Some("1h") => ("1h".into(), 1),
            Some("3h") => ("3h".into(), 2),
            Some("1d") => ("1d".into(), 3),
            Some("1w") => ("1w".into(), 4),
            Some(v) => (v.to_string(), 5),
        };
        Self {
            format: s.format.clone().unwrap_or_default(),
            original_name: s.original_name.unwrap_or(false),
            deletes_at,
            deletes_at_preset: preset,
            custom_amount: String::new(),
            custom_unit: 1, // hours default
            max_views: s.max_views.map(|v| v.to_string()).unwrap_or_default(),
            password: s.password.clone().unwrap_or_default(),
            folder_id: s.folder_id.clone().unwrap_or_default(),
            domain: s.domain.clone().unwrap_or_default(),
            image_compression_percent: s.image_compression_percent.map(|v| v.to_string()).unwrap_or_default(),
            image_compression_type: s.image_compression_type.clone().unwrap_or_default(),
        }
    }

    fn apply_to_settings(&self, s: &mut Settings) {
        s.format = if self.format.is_empty() || self.format == "(default)" { None } else { Some(self.format.clone()) };
        s.original_name = if self.original_name { Some(true) } else { None };
        s.deletes_at = if self.deletes_at.is_empty() { None } else { Some(self.deletes_at.clone()) };
        s.max_views = self.max_views.parse::<u32>().ok().filter(|&v| v > 0);
        s.password = if self.password.is_empty() { None } else { Some(self.password.clone()) };
        s.folder_id = if self.folder_id.is_empty() { None } else { Some(self.folder_id.clone()) };
        s.domain = if self.domain.is_empty() { None } else { Some(self.domain.clone()) };
        s.image_compression_percent = self.image_compression_percent.parse::<u32>().ok().filter(|&v| v > 0);
        s.image_compression_type = if self.image_compression_type.is_empty() { None } else { Some(self.image_compression_type.clone()) };
    }

    fn to_extra_headers(&self) -> Vec<(String, String)> {
        let mut h = Vec::new();
        if !self.format.is_empty() && self.format != "(default)" {
            h.push(("x-zipline-format".into(), self.format.clone()));
        }
        if self.original_name {
            h.push(("x-zipline-original-name".into(), "true".into()));
        }
        if !self.deletes_at.is_empty() {
            h.push(("x-zipline-deletes-at".into(), self.deletes_at.clone()));
        }
        if let Ok(v) = self.max_views.parse::<u32>() {
            if v > 0 { h.push(("x-zipline-max-views".into(), v.to_string())); }
        }
        if !self.password.is_empty() {
            h.push(("x-zipline-password".into(), self.password.clone()));
        }
        if !self.folder_id.is_empty() {
            h.push(("x-zipline-folder".into(), self.folder_id.clone()));
        }
        if !self.domain.is_empty() {
            h.push(("x-zipline-domain".into(), self.domain.clone()));
        }
        if let Ok(pct) = self.image_compression_percent.parse::<u32>() {
            if pct > 0 {
                h.push(("x-zipline-image-compression-percent".into(), pct.to_string()));
                h.push(("x-zipline-image-compression-type".into(), self.image_compression_type.clone()));
            }
        }
        h
    }
}

enum ShortenStatus {
    Idle,
    Working,
    Done(String),
    Error(String),
}

struct App {
    status: Status,
    queued: Option<PathBuf>,
    rx: Option<Receiver<UploadResult>>,
    kde: bool,
    tab: Tab,
    // Saved settings (written to disk on Save)
    saved_settings: Settings,
    // Settings tab — upload options edit state (kept separate so we can dirty-check)
    settings_opts: UploadOptions,
    settings_ext_text: String,
    // Per-upload overrides on the Upload tab
    per_upload: UploadOptions,
    show_per_upload: bool,
    // Shorten tab
    shorten_url: String,
    shorten_vanity: String,
    shorten_status: ShortenStatus,
    shorten_rx: Option<std::sync::mpsc::Receiver<Result<String, String>>>,
}

#[derive(PartialEq)]
enum Tab {
    Upload,
    Shorten,
    Settings,
}

impl App {
    fn new(kde: bool) -> Self {
        let s = Settings::load();
        let ext_text = s.allowed_extensions.as_ref().map(|v| v.join(", ")).unwrap_or_default();
        let opts = UploadOptions::from_settings(&s);
        let per_upload = opts.clone();
        Self {
            status: Status::Idle,
            queued: None,
            rx: None,
            kde,
            tab: Tab::Upload,
            saved_settings: s,
            settings_opts: opts,
            settings_ext_text: ext_text,
            per_upload,
            show_per_upload: false,
            shorten_url: String::new(),
            shorten_vanity: String::new(),
            shorten_status: ShortenStatus::Idle,
            shorten_rx: None,
        }
    }

    fn start_upload(&mut self, file: PathBuf, ctx: egui::Context) {
        if !self.saved_settings.extension_allowed(&file) {
            let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("unknown");
            self.status = Status::Blocked(format!(".{} is not in your allowed extensions list", ext));
            return;
        }

        self.status = Status::Uploading;
        let kde = self.kde;
        let extra_headers = self.per_upload.to_extra_headers();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);

        std::thread::spawn(move || {
            let cfg = config::load_or_setup();
            let result = match do_upload(&cfg, &file, extra_headers) {
                Ok(url) => {
                    let copied = copy_to_clipboard(&url);
                    notify::success(&file, &url, kde, copied);
                    UploadResult::Ok { url, copied }
                }
                Err(e) => {
                    logger::log_failure(&file, &e);
                    notify::failure(&file, &e);
                    UploadResult::Err(e.to_string())
                }
            };
            tx.send(result).ok();
            ctx.request_repaint();
        });
    }

    fn save_settings(&mut self) {
        self.settings_opts.apply_to_settings(&mut self.saved_settings);
        let exts: Vec<String> = self.settings_ext_text
            .split(|c| c == ',' || c == ' ')
            .map(|s: &str| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        self.saved_settings.allowed_extensions = if exts.is_empty() { None } else { Some(exts) };
        self.saved_settings.save();
        // Sync per-upload defaults from newly saved settings
        self.per_upload = UploadOptions::from_settings(&self.saved_settings);
    }
}

/// The actual HTTP upload — .sxcu headers first, extra headers layered on top.
fn do_upload(
    cfg: &crate::config::SxcuConfig,
    file: &std::path::Path,
    extra: Vec<(String, String)>,
) -> Result<String, upload::UploadError> {
    let auth = cfg
        .authorization()
        .ok_or_else(|| upload::UploadError("No authorization header in config".into()))?
        .to_string();

    let file_name = file.file_name().and_then(|n| n.to_str()).unwrap_or("upload").to_string();
    let mime = mime_guess::from_path(file).first_or_octet_stream().to_string();
    let bytes = std::fs::read(file).map_err(|e| upload::UploadError(format!("Could not read file: {}", e)))?;

    let part = reqwest::blocking::multipart::Part::bytes(bytes)
        .file_name(file_name)
        .mime_str(&mime)
        .map_err(|e| upload::UploadError(e.to_string()))?;

    let form = reqwest::blocking::multipart::Form::new().part(cfg.file_form_name.clone(), part);
    let client = reqwest::blocking::Client::new();
    let mut builder = client.post(&cfg.request_url).header("authorization", &auth).multipart(form);

    // .sxcu headers (skip authorization, already set)
    for (k, v) in &cfg.headers {
        if k.to_lowercase() != "authorization" {
            builder = builder.header(k.clone(), v.clone());
        }
    }

    // Extra headers — skip any already present in .sxcu
    let sxcu_keys: std::collections::HashSet<String> = cfg.headers.keys().map(|k| k.to_lowercase()).collect();
    for (k, v) in extra {
        if !sxcu_keys.contains(&k.to_lowercase()) {
            builder = builder.header(k, v);
        }
    }

    let response = builder.send().map_err(|e| upload::UploadError(format!("Request failed: {}", e)))?;
    let status = response.status();
    let body = response.text().unwrap_or_else(|_| "(no response body)".into());

    if !status.is_success() {
        return Err(upload::UploadError(format!("{} {}: {}", status.as_u16(), status.canonical_reason().unwrap_or(""), body)));
    }

    let json: serde_json::Value = serde_json::from_str(&body)
        .map_err(|_| upload::UploadError(format!("Invalid JSON response: {}", body)))?;

    json["files"][0]["url"].as_str().map(|s| s.to_string())
        .ok_or_else(|| upload::UploadError(format!("Could not find files[0].url in: {}", body)))
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(rx) = &self.rx {
            if let Ok(result) = rx.try_recv() {
                self.rx = None;
                self.status = match result {
                    UploadResult::Ok { url, copied } => Status::Done { url, copied },
                    UploadResult::Err(msg) => Status::Error(msg),
                };
            }
        }

        if let Some(rx) = &self.shorten_rx {
            if let Ok(result) = rx.try_recv() {
                self.shorten_rx = None;
                self.shorten_status = match result {
                    Ok(url) => ShortenStatus::Done(url),
                    Err(msg) => ShortenStatus::Error(msg),
                };
            }
        }

        ctx.input(|i| {
            if let Some(f) = i.raw.dropped_files.first() {
                if let Some(p) = &f.path {
                    self.queued = Some(p.clone());
                }
            }
        });

        if matches!(self.status, Status::Idle) {
            if let Some(file) = self.queued.take() {
                self.start_upload(file, ctx.clone());
            }
        }

        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Upload, "Upload");
                ui.selectable_value(&mut self.tab, Tab::Shorten, "Shorten");
                ui.selectable_value(&mut self.tab, Tab::Settings, "Settings");
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.tab {
                Tab::Upload => self.show_upload_tab(ui, ctx),
                Tab::Shorten => self.show_shorten_tab(ui, ctx),
                Tab::Settings => self.show_settings_tab(ui),
            }
        });
    }
}

impl App {
    fn show_upload_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space(12.0);
        ui.vertical_centered(|ui| {
            match &self.status {
                Status::Idle => {
                    let frame = egui::Frame::default()
                        .stroke(egui::Stroke::new(2.0, egui::Color32::from_gray(100)))
                        .inner_margin(egui::Margin::same(20.0))
                        .rounding(egui::Rounding::same(8.0));

                    frame.show(ui, |ui| {
                        ui.set_min_size(egui::vec2(380.0, 90.0));
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new("⬆  Drop a file here").size(20.0).color(egui::Color32::from_gray(170)));
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new("or").color(egui::Color32::from_gray(120)));
                            ui.add_space(8.0);
                            if ui.button("  Browse…  ").clicked() {
                                if let Some(path) = rfd::FileDialog::new().pick_file() {
                                    self.queued = Some(path);
                                }
                            }
                        });
                    });

                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Config:").size(11.0).color(egui::Color32::from_gray(140)));
                        ui.label(egui::RichText::new(config::config_display_path()).size(11.0).color(egui::Color32::from_gray(110)));
                        if ui.small_button("Change…").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title("Select .sxcu config")
                                .add_filter("ShareX Config", &["sxcu"])
                                .pick_file()
                            {
                                config::save_sxcu(&path);
                            }
                        }
                    });

                    ui.add_space(6.0);
                    ui.checkbox(&mut self.show_per_upload, "Override upload settings for this file");

                    if self.show_per_upload {
                        ui.add_space(6.0);
                        egui::Frame::default()
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(70)))
                            .inner_margin(egui::Margin::same(10.0))
                            .rounding(egui::Rounding::same(6.0))
                            .show(ui, |ui| {
                                ui.set_min_width(400.0);
                                // Use "pu_" prefix on all IDs to avoid conflict with settings tab
                                show_upload_options(ui, &mut self.per_upload, "pu");
                            });
                    }
                }

                Status::Uploading => {
                    ui.add_space(40.0);
                    ui.spinner();
                    ui.add_space(8.0);
                    ui.label("Uploading…");
                    ctx.request_repaint();
                }

                Status::Done { url, copied } => {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("✓ Uploaded").size(20.0).color(egui::Color32::from_rgb(80, 200, 120)));
                    ui.add_space(8.0);
                    let display = if url.len() > 50 { format!("{}…", &url[..50]) } else { url.clone() };
                    let url_clone = url.clone();
                    ui.label(egui::RichText::new(display).size(13.0));
                    ui.add_space(4.0);
                    if *copied {
                        ui.label(egui::RichText::new("Copied to clipboard").size(11.0).color(egui::Color32::from_gray(140)));
                    }
                    ui.add_space(12.0);
                    let mut copy_again = false;
                    let mut upload_another = false;
                    ui.horizontal(|ui| {
                        if ui.button("Copy again").clicked() { copy_again = true; }
                        if ui.button("Upload another").clicked() { upload_another = true; }
                    });
                    if copy_again { copy_to_clipboard(&url_clone); }
                    if upload_another {
                        self.status = Status::Idle;
                        self.per_upload = UploadOptions::from_settings(&self.saved_settings);
                        self.show_per_upload = false;
                    }
                }

                Status::Blocked(msg) => {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("⊘ Blocked").size(20.0).color(egui::Color32::from_rgb(220, 160, 40)));
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(msg).size(12.0).color(egui::Color32::from_gray(200)));
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("Add the extension in Settings → Allowed Extensions, or rename the file.").size(10.0).color(egui::Color32::from_gray(130)));
                    ui.add_space(12.0);
                    if ui.button("Go back").clicked() { self.status = Status::Idle; }
                }

                Status::Error(msg) => {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("✗ Upload failed").size(20.0).color(egui::Color32::from_rgb(220, 80, 80)));
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(msg).size(11.0).color(egui::Color32::from_gray(180)));
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("See ~/.config/zipline-upload/errors.log").size(10.0).color(egui::Color32::from_gray(120)));
                    ui.add_space(12.0);
                    if ui.button("Try again").clicked() { self.status = Status::Idle; }
                }
            }
        });
    }

    fn show_settings_tab(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(8.0);

            // App
            ui.label(egui::RichText::new("App").size(13.0).strong());
            ui.separator();
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Notification style");
                let kde = self.saved_settings.kde_notifications.get_or_insert(self.kde);
                egui::ComboBox::from_id_salt("notif_style")
                    .selected_text(if *kde { "KDE" } else { "Generic" })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(kde, true, "KDE");
                        ui.selectable_value(kde, false, "Generic");
                    });
            });

            ui.add_space(10.0);

            // Upload defaults
            ui.label(egui::RichText::new("Upload Defaults").size(13.0).strong());
            ui.add_space(2.0);
            ui.label(egui::RichText::new("Sent as headers on every upload unless already set in your .sxcu.").size(10.0).color(egui::Color32::from_gray(140)));
            ui.separator();
            ui.add_space(4.0);
            // Use "st_" prefix on all IDs to avoid conflict with per-upload panel
            show_upload_options(ui, &mut self.settings_opts, "st");

            ui.add_space(10.0);

            // Allowed extensions
            ui.label(egui::RichText::new("Allowed Extensions").size(13.0).strong());
            ui.add_space(2.0);
            ui.label(egui::RichText::new("Comma or space separated. Leave blank to allow everything.").size(10.0).color(egui::Color32::from_gray(140)));
            ui.separator();
            ui.add_space(4.0);
            ui.text_edit_singleline(&mut self.settings_ext_text);

            ui.add_space(14.0);
            if ui.button("  Save settings  ").clicked() {
                self.save_settings();
            }
            ui.add_space(8.0);
        });
    }

    fn show_shorten_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Poll shorten thread
        if self.shorten_rx.is_some() {
            ctx.request_repaint();
        }

        ui.add_space(16.0);
        ui.vertical_centered(|ui| {
            ui.heading("Shorten URL");
            ui.add_space(12.0);

            match &self.shorten_status {
                ShortenStatus::Idle | ShortenStatus::Error(_) => {
                    egui::Grid::new("shorten_grid")
                        .num_columns(2)
                        .spacing([8.0, 8.0])
                        .show(ui, |ui| {
                            ui.label("URL");
                            ui.add(egui::TextEdit::singleline(&mut self.shorten_url)
                                .desired_width(300.0)
                                .hint_text("https://example.com/very/long/url"));
                            ui.end_row();

                            ui.label("Vanity (optional)");
                            ui.add(egui::TextEdit::singleline(&mut self.shorten_vanity)
                                .desired_width(300.0)
                                .hint_text("my-link"));
                            ui.end_row();
                        });

                    if let ShortenStatus::Error(msg) = &self.shorten_status {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(msg).size(11.0).color(egui::Color32::from_rgb(220, 80, 80)));
                    }

                    ui.add_space(12.0);
                    let can_shorten = !self.shorten_url.is_empty();
                    ui.add_enabled_ui(can_shorten, |ui| {
                        if ui.button("  Shorten  ").clicked() {
                            self.do_shorten(ctx.clone());
                        }
                    });
                }

                ShortenStatus::Working => {
                    ui.add_space(40.0);
                    ui.spinner();
                    ui.add_space(8.0);
                    ui.label("Shortening…");
                    ctx.request_repaint();
                }

                ShortenStatus::Done(url) => {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("✓ Shortened").size(20.0).color(egui::Color32::from_rgb(80, 200, 120)));
                    ui.add_space(8.0);
                    let display = if url.len() > 50 { format!("{}…", &url[..50]) } else { url.clone() };
                    let url_clone = url.clone();
                    ui.label(egui::RichText::new(display).size(13.0));
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("Copied to clipboard").size(11.0).color(egui::Color32::from_gray(140)));
                    ui.add_space(14.0);
                    let mut copy_again = false;
                    let mut shorten_another = false;
                    ui.horizontal(|ui| {
                        if ui.button("Copy again").clicked() { copy_again = true; }
                        if ui.button("Shorten another").clicked() { shorten_another = true; }
                    });
                    if copy_again { copy_to_clipboard(&url_clone); }
                    if shorten_another {
                        self.shorten_status = ShortenStatus::Idle;
                        self.shorten_url.clear();
                        self.shorten_vanity.clear();
                    }
                }
            }
        });
    }

    fn do_shorten(&mut self, ctx: egui::Context) {
        self.shorten_status = ShortenStatus::Working;
        let destination = self.shorten_url.clone();
        let vanity = self.shorten_vanity.clone();
        let kde = self.kde;
        let (tx, rx) = std::sync::mpsc::channel();
        self.shorten_rx = Some(rx);

        std::thread::spawn(move || {
            let cfg = config::load_or_setup();
            let vanity_ref = if vanity.is_empty() { None } else { Some(vanity.as_str()) };
            let result = match shorten::shorten_url(&cfg, &destination, vanity_ref) {
                Ok(url) => {
                    let copied = copy_to_clipboard(&url);
                    notify::shorten_success(&url, kde, copied);
                    Ok(url)
                }
                Err(e) => {
                    notify::shorten_failure(&e);
                    Err(e.to_string())
                }
            };
            tx.send(result).ok();
            ctx.request_repaint();
        });
    }

}

/// Shared upload options grid. `id_prefix` must be unique per call site to avoid egui widget ID conflicts.
fn show_upload_options(ui: &mut egui::Ui, opts: &mut UploadOptions, id_prefix: &str) {
    egui::Grid::new(format!("{}_grid", id_prefix))
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.label("Filename format");
            egui::ComboBox::from_id_salt(format!("{}_fmt", id_prefix))
                .selected_text(if opts.format.is_empty() { "(default)" } else { &opts.format })
                .show_ui(ui, |ui| {
                    for &f in FORMATS {
                        ui.selectable_value(&mut opts.format, f.to_string(), f);
                    }
                });
            ui.end_row();

            ui.label("Keep original name");
            ui.checkbox(&mut opts.original_name, "");
            ui.end_row();

            ui.label("Auto-delete after");
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt(format!("{}_expiry", id_prefix))
                    .selected_text(EXPIRY_PRESETS[opts.deletes_at_preset])
                    .show_ui(ui, |ui| {
                        for (i, &p) in EXPIRY_PRESETS.iter().enumerate() {
                            ui.selectable_value(&mut opts.deletes_at_preset, i, p);
                        }
                    });
                match opts.deletes_at_preset {
                    0 => opts.deletes_at.clear(),
                    1 => opts.deletes_at = "1h".into(),
                    2 => opts.deletes_at = "3h".into(),
                    3 => opts.deletes_at = "1d".into(),
                    4 => opts.deletes_at = "1w".into(),
                    _ => {}
                }
                if opts.deletes_at_preset == 5 {
                    // Number input
                    ui.add(egui::TextEdit::singleline(&mut opts.custom_amount)
                        .desired_width(48.0)
                        .hint_text("1"));
                    // Unit dropdown
                    egui::ComboBox::from_id_salt(format!("{}_expiry_unit", id_prefix))
                        .selected_text(EXPIRY_UNITS[opts.custom_unit].0)
                        .show_ui(ui, |ui| {
                            for (i, &(label, _)) in EXPIRY_UNITS.iter().enumerate() {
                                ui.selectable_value(&mut opts.custom_unit, i, label);
                            }
                        });
                    // Build the deletes_at string
                    let amt = opts.custom_amount.trim();
                    if !amt.is_empty() {
                        opts.deletes_at = format!("{}{}", amt, EXPIRY_UNITS[opts.custom_unit].1);
                    } else {
                        opts.deletes_at.clear();
                    }
                }
            });
            ui.end_row();

            ui.label("Max views (0 = off)");
            ui.text_edit_singleline(&mut opts.max_views);
            ui.end_row();

            ui.label("Password");
            ui.text_edit_singleline(&mut opts.password);
            ui.end_row();

            ui.label("Folder ID");
            ui.text_edit_singleline(&mut opts.folder_id);
            ui.end_row();

            ui.label("Domain override");
            ui.text_edit_singleline(&mut opts.domain);
            ui.end_row();

            ui.label("Image compression %");
            ui.text_edit_singleline(&mut opts.image_compression_percent);
            ui.end_row();

            let pct: u32 = opts.image_compression_percent.parse().unwrap_or(0);
            if pct > 0 {
                ui.label("Compression format");
                egui::ComboBox::from_id_salt(format!("{}_ctype", id_prefix))
                    .selected_text(if opts.image_compression_type.is_empty() { "(server default)" } else { &opts.image_compression_type })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut opts.image_compression_type, String::new(), "(server default)");
                        for &t in COMPRESSION_TYPES {
                            ui.selectable_value(&mut opts.image_compression_type, t.to_string(), t);
                        }
                    });
                ui.end_row();
            }
        });
}
