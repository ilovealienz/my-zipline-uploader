use crate::{config, copy_to_clipboard, logger, notify, upload, AppFlags};
use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

pub fn run(flags: AppFlags) {
    let kde = flags.is_kde();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Zipline Upload")
            .with_inner_size([420.0, 300.0])
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
}

struct App {
    status: Status,
    queued: Option<PathBuf>,
    rx: Option<Receiver<UploadResult>>,
    kde: bool,
}

impl App {
    fn new(kde: bool) -> Self {
        Self { status: Status::Idle, queued: None, rx: None, kde }
    }

    fn start_upload(&mut self, file: PathBuf, ctx: egui::Context) {
        self.status = Status::Uploading;

        let kde = self.kde;
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);

        std::thread::spawn(move || {
            let cfg = config::load_or_setup();
            let result = match upload::upload_file(&cfg, &file) {
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
            // Wake the UI so it picks up the result immediately
            ctx.request_repaint();
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll upload thread result
        if let Some(rx) = &self.rx {
            if let Ok(result) = rx.try_recv() {
                self.rx = None;
                self.status = match result {
                    UploadResult::Ok { url, copied } => Status::Done { url, copied },
                    UploadResult::Err(msg) => Status::Error(msg),
                };
            }
        }

        // Queue a file from drag-and-drop
        ctx.input(|i| {
            if let Some(f) = i.raw.dropped_files.first() {
                if let Some(p) = &f.path {
                    self.queued = Some(p.clone());
                }
            }
        });

        // Kick off upload if something is queued and we're idle
        if matches!(self.status, Status::Idle) {
            if let Some(file) = self.queued.take() {
                self.start_upload(file, ctx.clone());
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                ui.heading("Zipline Upload");
                ui.add_space(12.0);

                match &self.status {
                    Status::Idle => {
                        let frame = egui::Frame::default()
                            .stroke(egui::Stroke::new(2.0, egui::Color32::from_gray(100)))
                            .inner_margin(egui::Margin::same(24.0))
                            .rounding(egui::Rounding::same(8.0));

                        frame.show(ui, |ui| {
                            ui.set_min_size(egui::vec2(340.0, 130.0));
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new("⬆  Drop a file here")
                                        .size(20.0)
                                        .color(egui::Color32::from_gray(170)),
                                );
                                ui.add_space(10.0);
                                ui.label(
                                    egui::RichText::new("or")
                                        .color(egui::Color32::from_gray(120)),
                                );
                                ui.add_space(10.0);
                                if ui.button("  Browse…  ").clicked() {
                                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                                        self.queued = Some(path);
                                    }
                                }
                            });
                        });

                        ui.add_space(14.0);

                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Config:")
                                    .size(11.0)
                                    .color(egui::Color32::from_gray(140)),
                            );
                            ui.label(
                                egui::RichText::new(config::config_display_path())
                                    .size(11.0)
                                    .color(egui::Color32::from_gray(110)),
                            );
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
                    }

                    Status::Uploading => {
                        ui.add_space(50.0);
                        ui.spinner();
                        ui.add_space(8.0);
                        ui.label("Uploading…");
                        // Keep repainting while upload is in progress
                        ctx.request_repaint();
                    }

                    Status::Done { url, copied } => {
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new("✓ Uploaded")
                                .size(20.0)
                                .color(egui::Color32::from_rgb(80, 200, 120)),
                        );
                        ui.add_space(8.0);
                        let display = if url.len() > 50 {
                            format!("{}…", &url[..50])
                        } else {
                            url.clone()
                        };
                        let url_clone = url.clone();
                        ui.label(egui::RichText::new(display).size(13.0));
                        ui.add_space(4.0);
                        if *copied {
                            ui.label(
                                egui::RichText::new("Copied to clipboard")
                                    .size(11.0)
                                    .color(egui::Color32::from_gray(140)),
                            );
                        }
                        ui.add_space(14.0);
                        let mut copy_again = false;
                        let mut upload_another = false;
                        ui.horizontal(|ui| {
                            if ui.button("Copy again").clicked() { copy_again = true; }
                            if ui.button("Upload another").clicked() { upload_another = true; }
                        });
                        if copy_again { copy_to_clipboard(&url_clone); }
                        if upload_another { self.status = Status::Idle; }
                    }

                    Status::Error(msg) => {
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new("✗ Upload failed")
                                .size(20.0)
                                .color(egui::Color32::from_rgb(220, 80, 80)),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(msg)
                                .size(11.0)
                                .color(egui::Color32::from_gray(180)),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("See ~/.config/zipline-upload/errors.log")
                                .size(10.0)
                                .color(egui::Color32::from_gray(120)),
                        );
                        ui.add_space(12.0);
                        if ui.button("Try again").clicked() {
                            self.status = Status::Idle;
                        }
                    }
                }
            });
        });
    }
}
