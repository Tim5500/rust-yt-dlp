use eframe::egui;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use crate::backend::{YtDlpBackend, Quality};

#[derive(Default)]
pub struct RustYtdlpApp {
    url: String,
    quality: Quality,
    output_dir: PathBuf,
    status: Arc<Mutex<String>>,
    progress: Arc<Mutex<f32>>,
    is_downloading: bool,
    history: Arc<Mutex<Vec<String>>>,
    cancel_flag: Arc<AtomicBool>,
}

impl RustYtdlpApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self::default();
        app.output_dir = crate::config::get_default_output_dir();
        app.status = Arc::new(Mutex::new("Ready. First run will automatically download yt-dlp...".to_string()));
        app.progress = Arc::new(Mutex::new(0.0));
        app.history = Arc::new(Mutex::new(Vec::new()));
        app.cancel_flag = Arc::new(AtomicBool::new(false));
        app
    }

    fn start_download(&mut self, ctx: &egui::Context) {
        self.is_downloading = true;
        *self.progress.lock().unwrap() = 0.0;
        self.cancel_flag.store(false, Ordering::Relaxed);

        let url = self.url.clone();
        let output_dir = self.output_dir.clone();
        let quality = self.quality;
        let status = Arc::clone(&self.status);
        let progress_callback = Arc::clone(&self.progress);
        let progress_reset = Arc::clone(&self.progress);
        let history = Arc::clone(&self.history);
        let cancel_flag = Arc::clone(&self.cancel_flag);

        let ctx_progress = ctx.clone();
        let ctx_finish = ctx.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let backend = YtDlpBackend::new();

            let result = rt.block_on(async {
                backend.download(&url, &output_dir, quality, 
                    move |p| {
                        *progress_callback.lock().unwrap() = p.percentage;
                        ctx_progress.request_repaint();   // 每次进度更新都请求重绘
                    }, 
                    cancel_flag
                ).await
            });

            {
                let mut st = status.lock().unwrap();
                match result {
                    Ok(msg) => {
                        *st = msg;
                        history.lock().unwrap().push(url);
                    }
                    Err(e) => *st = format!("❌ Error: {}", e),
                }
            }

            *progress_reset.lock().unwrap() = 0.0;
            ctx_finish.request_repaint();
        });
    }
}

impl eframe::App for RustYtdlpApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 自动重置下载状态
        if self.is_downloading {
            let prog = *self.progress.lock().unwrap();
            if prog >= 99.0 || prog == 0.0 {
                self.is_downloading = false;
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("rust-yt-dlp Video Downloader");
            ui.separator();

            ui.label("Video / Live / Playlist URL:");
            ui.text_edit_singleline(&mut self.url);

            ui.add_space(8.0);
            ui.label("Quality:");
            egui::ComboBox::from_label("")
                .selected_text(match self.quality {
                    Quality::Best => "Best quality + auto merge to mp4 (Recommended)",
                    Quality::BestVideo => "Best video only",
                    Quality::Medium => "Medium (720p)",
                    Quality::Low => "Low (480p)",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.quality, Quality::Best, "Best quality + auto merge to mp4 (Recommended)");
                    ui.selectable_value(&mut self.quality, Quality::BestVideo, "Best video only");
                    ui.selectable_value(&mut self.quality, Quality::Medium, "Medium (720p)");
                    ui.selectable_value(&mut self.quality, Quality::Low, "Low (480p)");
                });

            ui.add_space(8.0);
            ui.label(format!("Output folder: {}", self.output_dir.display()));
            if ui.button("Browse...").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.output_dir = path;
                }
            }

            ui.separator();

            // ==================== 进度条显示（重点优化） ====================
            let current_progress = *self.progress.lock().unwrap();
            if self.is_downloading || current_progress > 0.0 {
                ui.add(
                    egui::ProgressBar::new(current_progress / 100.0)
                        .text(format!("Downloading: {:.1}%", current_progress))
                        .animate(true)
                );
            }

            ui.horizontal(|ui| {
                let download_btn = ui.add_enabled(!self.is_downloading && !self.url.is_empty(), 
                    egui::Button::new("🚀 Start Download"));
                
                if download_btn.clicked() {
                    self.start_download(ctx);
                }

                if self.is_downloading {
                    if ui.button("⛔ Cancel Download").clicked() {
                        self.cancel_flag.store(true, Ordering::Relaxed);
                        *self.status.lock().unwrap() = "⛔ Cancelling download...".to_string();
                        ctx.request_repaint();
                    }
                }
            });

            if ui.button("🔄 Update yt-dlp").clicked() {
                let backend = YtDlpBackend::new();
                let _ = backend.force_update_yt_dlp();
                *self.status.lock().unwrap() = "✅ Old yt-dlp removed. It will auto-update on next download.".to_string();
            }

            ui.separator();

            ui.label("Download History:");
            let history_list = self.history.lock().unwrap().clone();
            egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                for (i, item) in history_list.iter().enumerate() {
                    ui.label(format!("{}. {}", i + 1, item));
                }
                if history_list.is_empty() {
                    ui.label("No downloads yet.");
                }
            });

            ui.separator();
            ui.label("Status:");
            let status_text = self.status.lock().unwrap().clone();
            ui.label(egui::RichText::new(status_text)
                .strong()
                .color(egui::Color32::from_rgb(0, 220, 80)));
        });
    }
}