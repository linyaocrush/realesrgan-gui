//! The egui/eframe application: window layout, widget state and job control.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use egui::Ui;
use raw_window_handle::HasWindowHandle;

use crate::config::AppConfig;
use crate::define;
use crate::i18n::I18n;
use crate::image_util;
use crate::models;
use crate::params::{model_factor, Downsample, ReConfigParams, ResizeMode, TILE_SIZES};
use crate::task::{
    self, CustomCompressTask, GifSplitTask, LossyCompressTask, ReSpawnTask, Task, TaskCtx, UiMsg,
};
use crate::theme;
use crate::win;

#[derive(PartialEq, Eq, Clone, Copy)]
enum Tab {
    Basic,
    Advanced,
    About,
}

pub struct App {
    i18n: I18n,
    config: AppConfig,
    models: Vec<String>,
    model_factors: HashMap<String, i64>,
    model_index: usize,
    model_dir: PathBuf,
    re_path: Option<PathBuf>,
    realcugan: bool,
    upscayl: bool,

    input_paths: Vec<PathBuf>,
    input_text: String,
    output_text: String,
    output_path_changed: bool,

    log: String,
    log_file: Option<std::fs::File>,
    progress: f64,
    processing: bool,
    paused: bool,
    tab: Tab,

    tx: Sender<UiMsg>,
    rx: Receiver<UiMsg>,
    ctx: Option<Arc<TaskCtx>>,
    _tempdir: Option<tempfile::TempDir>,

    taskbar: Option<win::TaskbarHandle>,
    hwnd: Option<isize>,
    dark: bool,
    applied_dark: Option<bool>,
    /// Title bar theme last pushed to the OS.
    titlebar_dark: Option<bool>,
    /// While set (and in the future) the title bar theme is re-applied on every
    /// repaint: Windows resets the caption when the window is first mapped, so a
    /// single early call is not enough.
    titlebar_until: Option<Instant>,
    about_icon: Option<egui::TextureHandle>,
    last_theme_check: Instant,
    dialog: Option<(String, String)>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_inputs: Vec<PathBuf>) -> Self {
        let mut i18n = I18n::new();
        let config = AppConfig::load(&define::config_path(), &i18n);
        i18n.set_language(&config.app_language);

        let re_path = define::detect_re_path(Some(&config.upscaler));
        let realcugan = re_path.as_deref().map(define::is_realcugan).unwrap_or(false);
        let upscayl = re_path.as_deref().map(define::is_upscayl).unwrap_or(false);

        let model_dir = if config.model_dir.trim().is_empty() {
            define::default_models_dir()
        } else {
            PathBuf::from(config.model_dir.clone())
        };
        let mut models = models::list_models(&model_dir, realcugan);
        models::reorder_preferred(&mut models);
        let model_factors = models.iter().map(|m| (m.clone(), model_factor(m))).collect();
        let model_index = models
            .iter()
            .position(|m| *m == config.model)
            .unwrap_or(0);

        let (tx, rx) = crossbeam_channel::unbounded();
        let dark = matches!(win::detect_theme(), win::ThemeMode::Dark);
        theme::apply(&cc.egui_ctx, dark);
        let about_icon = load_about_icon(&cc.egui_ctx);

        let mut app = App {
            i18n,
            config,
            models,
            model_factors,
            model_index,
            model_dir,
            re_path,
            realcugan,
            upscayl,
            input_paths: Vec::new(),
            input_text: String::new(),
            output_text: String::new(),
            output_path_changed: true,
            log: String::new(),
            log_file: None,
            progress: 0.0,
            processing: false,
            paused: false,
            tab: Tab::Basic,
            tx,
            rx,
            ctx: None,
            _tempdir: None,
            taskbar: win::create_taskbar(),
            hwnd: None,
            dark,
            applied_dark: Some(dark),
            titlebar_dark: None,
            titlebar_until: Some(Instant::now() + std::time::Duration::from_millis(1500)),
            about_icon,
            last_theme_check: Instant::now(),
            dialog: None,
        };

        if !app.config.model_dir.trim().is_empty() {
            app.write_log(&format!(
                "Using custom model dir: {}\n",
                app.config.model_dir
            ));
        }
        if !app.config.upscaler.trim().is_empty() {
            app.write_log(&format!(
                "Using custom upscaler executable: {}\nThe executable (and models) may be incompatible with Real-ESRGAN-ncnn-vulkan. Use at your own risk!\n",
                app.config.upscaler
            ));
        }

        if !initial_inputs.is_empty() {
            app.set_input_path(initial_inputs);
        }
        app
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn write_log(&mut self, s: &str) {
        self.log.push_str(s);
        if let Some(f) = &mut self.log_file {
            use std::io::Write;
            let _ = f.write_all(s.as_bytes());
        }
    }

    fn warn(&mut self, key: &str) {
        let msg = self.i18n.tr(key);
        self.dialog = Some((self.i18n.tr("FrameBasicConfig"), msg));
    }

    fn write_config(&self) {
        if let Err(e) = self.config.save(&define::config_path()) {
            log::warn!("failed to save config: {e}");
        }
    }

    fn resize_value(&self) -> i64 {
        match ResizeMode::from_i64(self.config.resize_mode) {
            ResizeMode::Ratio => self.config.resize_ratio,
            ResizeMode::Width => self.config.resize_width,
            ResizeMode::Height => self.config.resize_height,
            ResizeMode::LongestSide => self.config.resize_longest_side,
            ResizeMode::ShortestSide => self.config.resize_shortest_side,
        }
    }

    fn current_model(&self) -> String {
        self.models.get(self.model_index).cloned().unwrap_or_default()
    }

    fn config_params(&self) -> ReConfigParams {
        let model = self.current_model();
        ReConfigParams {
            model_factor: self.model_factors.get(&model).copied().unwrap_or(4),
            model,
            model_dir: self.model_dir.clone(),
            resize_mode: ResizeMode::from_i64(self.config.resize_mode),
            resize_mode_value: self.resize_value(),
            downsample: Downsample::from_index(self.config.downsample_index),
            tile_size: TILE_SIZES
                .get(self.config.tile_size_index)
                .copied()
                .unwrap_or(0),
            gpu_id: self.config.gpu_id,
            use_tta: self.config.use_tta,
            preupscale: self.config.preupscale,
        }
    }

    fn set_input_path(&mut self, paths: Vec<PathBuf>) {
        self.input_paths = paths;
        self.input_text = self
            .input_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" | ");
        self.refresh_output_path();
    }

    /// Build the default output path list, mirroring the Python `getOutputPath`.
    fn refresh_output_path(&mut self) {
        if self.input_paths.is_empty() {
            return;
        }
        let model = self.current_model();
        let mode = ResizeMode::from_i64(self.config.resize_mode);
        let suffix = mode.suffix(self.resize_value());
        let custom = !self.config.custom_command.trim().is_empty();
        let out: Vec<String> = self
            .input_paths
            .iter()
            .map(|p| {
                let (base, ext) = split_output_base(p, custom, self.config.use_webp);
                format!("{base} ({model} {suffix}){ext}")
            })
            .collect();
        self.output_text = out.join(" | ");
        self.output_path_changed = false;
    }

    // -----------------------------------------------------------------------
    // Job control
    // -----------------------------------------------------------------------

    fn start(&mut self) {
        let input_paths: Vec<PathBuf> = split_paths(&self.input_text);
        let output_paths: Vec<PathBuf> = split_paths(&self.output_text);
        if input_paths.is_empty() || input_paths.len() != output_paths.len() {
            self.warn("WarningInvalidPath");
            return;
        }

        let params = self.config_params();
        if params.resize_mode == ResizeMode::Ratio && params.resize_mode_value < 2 {
            self.warn("WarningResizeRatio");
            return;
        }
        if self.re_path.is_none() {
            self.warn("WarningNotFoundRE");
            return;
        }

        let tempdir = match tempfile::Builder::new().prefix("realesrgan-gui-").tempdir() {
            Ok(d) => d,
            Err(e) => {
                self.dialog = Some(("Error".to_string(), e.to_string()));
                return;
            }
        };

        let ctx = Arc::new(TaskCtx::new(
            params,
            self.re_path.clone().unwrap(),
            self.realcugan,
            self.upscayl,
            self.config.optimize_gif,
            self.config.custom_command.trim().to_string(),
            self.tx.clone(),
            tempdir.path().to_path_buf(),
        ));

        let custom = !ctx.custom_command.is_empty();
        let use_webp = self.config.use_webp;
        let lossy = self.config.lossy_mode;
        let quality = self.config.lossy_quality;
        let mut tasks: Vec<Box<dyn Task>> = Vec::new();
        let mut total = 0usize;

        for (input, output) in input_paths.iter().zip(output_paths.iter()) {
            if !input.exists() {
                self.warn("WarningNotFoundPath");
                return;
            }
            if input.is_dir() {
                let mut files = Vec::new();
                collect_images(input, &mut files);
                if files.is_empty() {
                    self.warn("WarningEmptyFolder");
                    return;
                }
                for file in files {
                    let rel = file.strip_prefix(input).unwrap_or(&file);
                    let target = output.join(rel);
                    build_file_tasks(
                        &ctx, &mut tasks, &file, &target, custom, use_webp, lossy, quality,
                    );
                    total += 1;
                }
            } else if image_util::is_supported_image(input) {
                build_file_tasks(
                    &ctx, &mut tasks, input, output, custom, use_webp, lossy, quality,
                );
                total += 1;
            } else {
                self.warn("WarningInvalidFormat");
                return;
            }
        }

        if tasks.is_empty() {
            return;
        }

        ctx.set_total(total);
        ctx.pause.store(true, Ordering::SeqCst);
        ctx.running.store(true, Ordering::SeqCst);

        self.log.clear();
        self.log_file = std::fs::File::create(define::app_path().join("output.log")).ok();
        self.progress = 0.0;
        self.processing = true;
        self.paused = false;
        self.ctx = Some(ctx.clone());
        self._tempdir = Some(tempdir);

        if let (Some(tbl), Some(hwnd)) = (&self.taskbar, self.hwnd) {
            win::set_taskbar_progress(tbl, hwnd, win::ProgressState::Normal, 1, 0xFFFF_FFFF);
        }

        let ignore_error = self.config.ignore_error;
        std::thread::spawn(move || task::run_queue(tasks, ctx, ignore_error));
    }

    fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        if let Some(ctx) = &self.ctx {
            ctx.pause.store(!self.paused, Ordering::SeqCst);
        }
        if self.paused {
            self.write_log("Will pause after current task is completed.\n");
        }
    }

    fn poll_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                UiMsg::Log(s) => self.write_log(&s),
                UiMsg::Progress(p) => {
                    self.progress = p;
                    if let (Some(tbl), Some(hwnd)) = (&self.taskbar, self.hwnd) {
                        let state = if self.paused {
                            win::ProgressState::Paused
                        } else {
                            win::ProgressState::Normal
                        };
                        win::set_taskbar_progress(tbl, hwnd, state, p.round() as u64, 100);
                    }
                }
                UiMsg::Done { with_error, elapsed } => {
                    self.progress = 100.0;
                    let title = self.i18n.tr("ToastCompletedTitle");
                    let body = if with_error {
                        fill(
                            &self.i18n.tr("ToastCompletedMessageWithError"),
                            &define::app_path().join("output.log").to_string_lossy(),
                            "",
                        )
                    } else {
                        fill(
                            &self.i18n.tr("ToastCompletedMessage"),
                            &self.output_text,
                            &format!("{elapsed:.3}"),
                        )
                    };
                    win::notify(&title, &body);
                }
                UiMsg::Failed(e) => {
                    let title = self.i18n.tr("ToastFailedTitle");
                    win::notify(&title, &e);
                    if let (Some(tbl), Some(hwnd)) = (&self.taskbar, self.hwnd) {
                        win::set_taskbar_progress(tbl, hwnd, win::ProgressState::Error, 100, 100);
                    }
                }
                UiMsg::Finished => {
                    self.processing = false;
                    self.paused = false;
                    if let (Some(tbl), Some(hwnd)) = (&self.taskbar, self.hwnd) {
                        win::set_taskbar_progress(tbl, hwnd, win::ProgressState::None, 0, 0);
                    }
                    self.ctx = None;
                    self._tempdir = None;
                    self.write_config();
                }
            }
        }
    }

    fn check_theme(&mut self) {
        if self.last_theme_check.elapsed() < Duration::from_millis(1000) {
            return;
        }
        self.last_theme_check = Instant::now();
        let mode = win::detect_theme();
        let dark = match mode {
            win::ThemeMode::Dark => true,
            win::ThemeMode::Light => false,
            win::ThemeMode::Unknown => return,
        };
        if dark != self.dark {
            self.dark = dark;
            if let Some(hwnd) = self.hwnd {
                win::set_dark_titlebar(hwnd, dark);
            }
        }
    }

    // -----------------------------------------------------------------------
    // UI
    // -----------------------------------------------------------------------

    fn tab_basic(&mut self, ui: &mut Ui) {
        let models = self.models.clone();
        let has_models = !models.is_empty();
        let has_re = self.re_path.is_some();

        let l_input = self.i18n.tr("Input");
        let l_output = self.i18n.tr("Output");
        let l_browse = self.i18n.tr("OpenFileDialog");
        let l_resize = self.i18n.tr("ResizeMode");
        let l_model = self.i18n.tr("UsedModel");
        let l_ratio = self.i18n.tr("ResizeModeRatio");
        let l_width = self.i18n.tr("ResizeModeWidth");
        let l_height = self.i18n.tr("ResizeModeHeight");
        let l_longest = self.i18n.tr("ResizeModeLongestSide");
        let l_shortest = self.i18n.tr("ResizeModeShortestSide");

        ui.add_space(6.0);

        // Input -----------------------------------------------------------------
        ui.label(l_input);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::button(ui, &l_browse).clicked() {
                    if let Some(files) = rfd::FileDialog::new()
                        .add_filter(
                            "Image files",
                            &["jpg", "jpeg", "png", "gif", "webp", "tif", "tiff"],
                        )
                        .pick_files()
                    {
                        self.set_input_path(files);
                    }
                }
                let width = ui.available_width();
                let r = theme::text_field(ui, &mut self.input_text, width);
                if r.changed() {
                    self.input_paths = split_paths(&self.input_text);
                    self.output_path_changed = true;
                }
            });
        });

        ui.add_space(2.0);

        // Output ----------------------------------------------------------------
        ui.label(l_output);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::button(ui, &l_browse).clicked() {
                    if let Some(file) = rfd::FileDialog::new()
                        .add_filter("Image files", &["png", "gif", "webp"])
                        .save_file()
                    {
                        self.output_text = file.to_string_lossy().to_string();
                        self.output_path_changed = true;
                    }
                }
                let width = ui.available_width();
                theme::text_field(ui, &mut self.output_text, width);
            });
        });

        ui.add_space(8.0);

        // Resize mode (left) + model (right) ------------------------------------
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_max_width(238.0);
                ui.label(l_resize);
                let mut changed = false;
                egui::Grid::new("resize_grid")
                    .num_columns(2)
                    .min_col_width(128.0)
                    .spacing([8.0, 5.0])
                    .show(ui, |ui| {
                        let mode = &mut self.config.resize_mode;
                        changed |= theme::radio(ui, &mut *mode, 1, &l_ratio).changed();
                        changed |= theme::spin(ui, &mut self.config.resize_ratio, 2..=16, 110.0)
                            .changed();
                        ui.end_row();
                        changed |= theme::radio(ui, &mut *mode, 2, &l_width).changed();
                        changed |= theme::spin(ui, &mut self.config.resize_width, 1..=16383, 110.0)
                            .changed();
                        ui.end_row();
                        changed |= theme::radio(ui, &mut *mode, 3, &l_height).changed();
                        changed |= theme::spin(ui, &mut self.config.resize_height, 1..=16383, 110.0)
                            .changed();
                        ui.end_row();
                        changed |= theme::radio(ui, &mut *mode, 4, &l_longest).changed();
                        changed |=
                            theme::spin(ui, &mut self.config.resize_longest_side, 1..=16383, 110.0)
                                .changed();
                        ui.end_row();
                        changed |= theme::radio(ui, &mut *mode, 5, &l_shortest).changed();
                        changed |=
                            theme::spin(ui, &mut self.config.resize_shortest_side, 1..=16383, 110.0)
                                .changed();
                        ui.end_row();
                    });
                if changed && !self.output_path_changed {
                    self.refresh_output_path();
                }
            });

            ui.add_space(12.0);

            ui.vertical(|ui| {
                ui.label(l_model);
                let width = ui.available_width().max(160.0);
                theme::combo("model", width)
                    .selected_text(models.get(self.model_index).cloned().unwrap_or_default())
                    .show_ui(ui, |ui| {
                        for (i, m) in models.iter().enumerate() {
                            ui.selectable_value(&mut self.model_index, i, m);
                        }
                    });
                if !has_re {
                    ui.add_space(2.0);
                    ui.colored_label(theme::danger(), self.i18n.tr("WarningNotFoundRE"));
                    if ui.button("Real-ESRGAN releases").clicked() {
                        ui.ctx()
                            .open_url(egui::OpenUrl::new_tab(define::releases_url()));
                    }
                } else if !has_models {
                    ui.add_space(2.0);
                    ui.colored_label(theme::danger(), "No models found");
                }
            });
        });

        ui.add_space(8.0);

        // Start / pause button, bottom right -------------------------------------
        let label = if self.processing {
            if self.paused {
                self.i18n.tr("ContinueProcessing")
            } else {
                self.i18n.tr("PauseProcessing")
            }
        } else {
            self.i18n.tr("StartProcessing")
        };
        let enabled = self.processing || (has_re && has_models);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if theme::accent_button(ui, &label, enabled).clicked() {
                if self.processing {
                    self.toggle_pause();
                } else {
                    self.start();
                }
            }
        });
    }

    fn tab_advanced(&mut self, ui: &mut Ui) {
        let l_down = self.i18n.tr("DownsampleMode");
        let l_tile = self.i18n.tr("TileSize");
        let l_auto = self.i18n.tr("TileSizeAuto");
        let l_gpu = self.i18n.tr("UsedGPUID");
        let l_quality = self.i18n.tr("LossyModeQuality");
        let l_cmd = self.i18n.tr("CustomCommand");
        let l_webp = self.i18n.tr("PreferWebP");
        let l_tta = self.i18n.tr("EnableTTA");
        let l_gif = self.i18n.tr("GIFOptimizeTransparency");
        let l_lossy = self.i18n.tr("EnableLossyMode");
        let l_ignore = self.i18n.tr("EnableIgnoreError");
        let l_pre = self.i18n.tr("EnablePreupscale");

        ui.add_space(6.0);
        ui.horizontal_top(|ui| {
            // Left column: algorithm / tile / GPU / quality / custom command.
            ui.vertical(|ui| {
                ui.set_max_width(320.0);
                ui.columns(2, |cols| {
                    let w = (cols[0].available_width() - 4.0).max(80.0);
                    cols[0].label(l_down.as_str());
                    theme::combo("downsample", w)
                        .selected_text(Downsample::from_index(self.config.downsample_index).name())
                        .show_ui(&mut cols[0], |ui| {
                            for (i, (_, name)) in Downsample::ALL.iter().enumerate() {
                                ui.selectable_value(&mut self.config.downsample_index, i, *name);
                            }
                        });

                    let w = (cols[1].available_width() - 4.0).max(80.0);
                    cols[1].label(l_tile.as_str());
                    let tile_label = if self.config.tile_size_index == 0 {
                        l_auto.clone()
                    } else {
                        TILE_SIZES
                            .get(self.config.tile_size_index)
                            .copied()
                            .unwrap_or(0)
                            .to_string()
                    };
                    theme::combo("tile", w)
                        .selected_text(tile_label)
                        .show_ui(&mut cols[1], |ui| {
                            for (i, size) in TILE_SIZES.iter().enumerate() {
                                let label = if i == 0 {
                                    l_auto.clone()
                                } else {
                                    size.to_string()
                                };
                                ui.selectable_value(&mut self.config.tile_size_index, i, label);
                            }
                        });
                });

                ui.add_space(6.0);
                ui.label(l_gpu.as_str());
                let w = (ui.available_width() - 4.0).max(80.0);
                theme::spin(ui, &mut self.config.gpu_id, -1..=7, w);

                ui.add_space(6.0);
                ui.label(l_quality.as_str());
                let w = (ui.available_width() - 4.0).max(80.0);
                theme::spin(ui, &mut self.config.lossy_quality, 0..=100, w);

                ui.add_space(6.0);
                ui.label(l_cmd.as_str());
                let w = (ui.available_width() - 4.0).max(80.0);
                theme::text_field(ui, &mut self.config.custom_command, w);
            });

            ui.add_space(24.0);

            // Right column: check boxes and the language selector.
            ui.vertical(|ui| {
                theme::switch(ui, &mut self.config.use_webp, l_webp.as_str());
                theme::switch(ui, &mut self.config.use_tta, l_tta.as_str());
                theme::switch(ui, &mut self.config.optimize_gif, l_gif.as_str());
                theme::switch(ui, &mut self.config.lossy_mode, l_lossy.as_str());
                theme::switch(ui, &mut self.config.ignore_error, l_ignore.as_str());
                theme::switch(ui, &mut self.config.preupscale, l_pre.as_str());

                ui.add_space(10.0);
                let names = self.i18n.locale_names.clone();
                let current = self.i18n.current_index();
                let w = ui.available_width().max(160.0);
                theme::combo("language", w)
                    .selected_text(names.get(current).cloned().unwrap_or_default())
                    .show_ui(ui, |ui| {
                        for (i, name) in names.iter().enumerate() {
                            if ui.selectable_label(i == current, name.as_str()).clicked() {
                                if let Some(code) = self.i18n.locales.get(i).cloned() {
                                    self.i18n.set_language(&code);
                                    self.config.app_language = code;
                                }
                            }
                        }
                    });
            });
        });
    }

    fn tab_about(&mut self, ui: &mut Ui) {
        let urls: [(String, &'static str); 4] = [
            (self.i18n.tr("ViewREGUISource"), define::source_url()),
            (self.i18n.tr("ViewRESource"), define::re_intro_url()),
            (
                self.i18n.tr("ViewAdditionalModel"),
                define::additional_model_url(),
            ),
            (self.i18n.tr("ViewDonatePage"), define::donate_url()),
        ];
        let icon = self.about_icon.clone();
        ui.vertical_centered(|ui| {
            // Python centres the whole About block (.place(relx=.5, rely=.5, anchor=CENTER)).
            const CONTENT_H: f32 = 280.0;
            let pad = ((ui.available_height() - CONTENT_H) * 0.5).max(12.0);
            ui.add_space(pad);
            if let Some(tex) = &icon {
                ui.image(egui::load::SizedTexture::from_handle(tex));
                ui.add_space(6.0);
            }
            ui.label(egui::RichText::new(define::APP_TITLE).size(16.0));
            let mut by = String::from("By TransparentLC");
            if let Some(t) = define::BUILD_TIME {
                by.push_str(&format!("\nBuilt at {t}"));
            }
            ui.label(by);
            ui.add_space(12.0);

            // Two centred columns of equally sized buttons (Python: frameAboutBottom).
            const BW: f32 = 150.0;
            const BH: f32 = 26.0;
            const GAP: f32 = 8.0;
            const COLS: usize = 2;
            let block_w = COLS as f32 * BW + (COLS - 1) as f32 * GAP;
            let rows = urls.len().div_ceil(COLS);
            let block_h = rows as f32 * BH + (rows - 1) as f32 * GAP;
            ui.allocate_ui_with_layout(
                egui::vec2(block_w, block_h),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    for chunk in urls.chunks(COLS) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = GAP;
                            for (label, url) in chunk {
                                if ui
                                    .add_sized([BW, BH], egui::Button::new(label.as_str()))
                                    .clicked()
                                {
                                    ui.ctx().open_url(egui::OpenUrl::new_tab(*url));
                                }
                            }
                        });
                    }
                },
            );
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        if self.hwnd.is_none() {
            if let Ok(handle) = frame.window_handle() {
                if let raw_window_handle::RawWindowHandle::Win32(w) = handle.as_raw() {
                    self.hwnd = Some(w.hwnd.get());
                    win::set_dark_titlebar(w.hwnd.get(), self.dark);
                }
            }
        }

        // Handle drag and drop.
        let dropped: Vec<PathBuf> = ui.ctx().input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| {
                    let p = f.path().to_path_buf();
                    (!p.as_os_str().is_empty()).then_some(p)
                })
                .collect()
        });
        if !dropped.is_empty() {
            self.set_input_path(dropped);
        }

        self.poll_messages();
        self.check_theme();

        if ui.ctx().input(|i| i.viewport().close_requested()) {
            self.write_config();
        }

        if self.applied_dark != Some(self.dark) {
            theme::apply(ui.ctx(), self.dark);
            self.applied_dark = Some(self.dark);
            self.titlebar_until = Some(Instant::now() + std::time::Duration::from_millis(1500));
        }

        // Match the window frame (title bar) to the app theme. Windows resets the
        // caption colour while the window is being mapped, so keep re-applying for a
        // short while after a theme change instead of relying on a single call.
        if let Some(hwnd) = self.hwnd {
            let pending = self.titlebar_until.is_some_and(|t| Instant::now() < t);
            if pending || self.titlebar_dark != Some(self.dark) {
                win::set_dark_titlebar(hwnd, self.dark);
                self.titlebar_dark = Some(self.dark);
            }
            if pending {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(250));
            }
        }

        // Thin determinate progress bar at the very bottom (Python: ttk.Progressbar).
        egui::Panel::bottom("progress_panel")
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.add(
                    egui::ProgressBar::new((self.progress / 100.0) as f32)
                        .desired_height(16.0)
                        .fill(theme::palette(self.dark).progress)
                        .corner_radius(3),
                );
                ui.add_space(4.0);
            });

        // Log area above the progress bar, fixed height (Python: ScrolledText textOutput).
        egui::Panel::bottom("log_panel")
            .resizable(false)
            .exact_size(200.0)
            .frame(
                egui::Frame::side_top_panel(ui.style())
                    .fill(theme::palette(self.dark).entry_bg)
                    .stroke(egui::Stroke::new(
                        1.0,
                        theme::palette(self.dark).entry_border,
                    ))
                    .inner_margin(egui::Margin::same(6)),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&self.log).monospace().size(12.0),
                            )
                            .wrap(),
                        );
                    });
            });

        // Notebook-style tab strip (Python: ttk.Notebook tabs).
        egui::Panel::top("tabs_panel")
            .frame(
                egui::Frame::side_top_panel(ui.style())
                    .fill(theme::palette(self.dark).bg)
                    .inner_margin(egui::Margin {
                        left: 6,
                        right: 6,
                        top: 6,
                        bottom: 0,
                    }),
            )
            .show(ui, |ui| {
                let labels = [
                    self.i18n.tr("FrameBasicConfig"),
                    self.i18n.tr("FrameAdvancedConfig"),
                    self.i18n.tr("FrameAbout"),
                ];
                let mut index = match self.tab {
                    Tab::Basic => 0,
                    Tab::Advanced => 1,
                    Tab::About => 2,
                };
                if theme::tab_bar(ui, &mut index, &labels) {
                    self.tab = match index {
                        1 => Tab::Advanced,
                        2 => Tab::About,
                        _ => Tab::Basic,
                    };
                }
            });

        // Tab content fills everything between the tab strip and the log area.
        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(ui.style())
                    .fill(theme::palette(self.dark).bg)
                    .stroke(egui::Stroke::new(
                        1.0,
                        theme::palette(self.dark).border,
                    ))
                    .inner_margin(egui::Margin::same(8)),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.tab {
                        Tab::Basic => self.tab_basic(ui),
                        Tab::Advanced => self.tab_advanced(ui),
                        Tab::About => self.tab_about(ui),
                    });
            });

        if let Some((title, msg)) = self.dialog.clone() {
            egui::Window::new(title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui| {
                    ui.label(msg);
                    ui.add_space(8.0);
                    if ui.button("OK").clicked() {
                        self.dialog = None;
                    }
                });
        }
    }
}

// ---------------------------------------------------------------------------
// Free helpers
// ---------------------------------------------------------------------------

fn split_paths(text: &str) -> Vec<PathBuf> {
    text.split('|')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// Base path and adjusted extension used to name a default output file.
fn split_output_base(path: &Path, custom: bool, use_webp: bool) -> (String, String) {
    if path.is_dir() {
        return (path.to_string_lossy().to_string(), String::new());
    }
    let base = path
        .with_extension("")
        .to_string_lossy()
        .trim_end_matches('.')
        .to_string();
    let mut ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext == "jpg" || ext == "tif" || ext == "tiff" || custom {
        ext = "png".to_string();
    }
    if ext == "png" && use_webp {
        ext = "webp".to_string();
    }
    (base, if ext.is_empty() { String::new() } else { format!(".{ext}") })
}

fn adjusted_ext(path: &Path, custom: bool, use_webp: bool) -> String {
    let (_, ext) = split_output_base(path, custom, use_webp);
    ext.trim_start_matches('.').to_string()
}

fn collect_images(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_images(&path, out);
            } else if image_util::is_supported_image(&path) {
                out.push(path);
            }
        }
    }
}

/// Create the task chain for a single input file.
#[allow(clippy::too_many_arguments)]
fn build_file_tasks(
    ctx: &TaskCtx,
    tasks: &mut Vec<Box<dyn Task>>,
    input: &Path,
    output: &Path,
    custom: bool,
    use_webp: bool,
    lossy: bool,
    quality: i64,
) {
    if image_util::ext_is(input, &["gif"]) {
        tasks.push(Box::new(GifSplitTask {
            input: input.to_path_buf(),
            output: output.to_path_buf(),
        }));
        return;
    }

    let ext = adjusted_ext(input, custom, use_webp);
    let final_path = if output.extension().is_none() || output.is_dir() {
        output.with_extension(&ext)
    } else {
        output.to_path_buf()
    };

    if custom {
        let temp = ctx.temp_path("png");
        tasks.push(Box::new(ReSpawnTask::new(
            input.to_path_buf(),
            temp.clone(),
            false,
        )));
        tasks.push(Box::new(CustomCompressTask {
            input: temp,
            output: final_path,
            template: ctx.custom_command.clone(),
            remove_input: true,
        }));
    } else if lossy && matches!(ext.as_str(), "jpg" | "jpeg" | "webp") {
        let temp = ctx.temp_path("webp");
        tasks.push(Box::new(ReSpawnTask::new(
            input.to_path_buf(),
            temp.clone(),
            false,
        )));
        tasks.push(Box::new(LossyCompressTask {
            input: temp,
            output: final_path,
            quality,
            remove_input: true,
        }));
    } else {
        tasks.push(Box::new(ReSpawnTask::new(
            input.to_path_buf(),
            final_path,
            false,
        )));
    }
}

/// Substitute `{0}` / `{1}` (and the `{1:.03f}` form) in a translated message.
fn fill(template: &str, a0: &str, a1: &str) -> String {
    template
        .replace("{0}", a0)
        .replace("{1:.03f}", a1)
        .replace("{1}", a1)
}

/// Load `icon-128px.png` into a texture for the "About" tab.
fn load_about_icon(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let bytes: &[u8] = include_bytes!("../../icon-128px.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    Some(ctx.load_texture("about_icon", color, egui::TextureOptions::LINEAR))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_ext_mapping() {
        assert_eq!(split_output_base(Path::new("a/b/photo.jpg"), false, false).1, ".png");
        assert_eq!(split_output_base(Path::new("photo.tiff"), false, false).1, ".png");
        assert_eq!(split_output_base(Path::new("photo.png"), false, true).1, ".webp");
        assert_eq!(split_output_base(Path::new("photo.webp"), false, true).1, ".webp");
        assert_eq!(split_output_base(Path::new("photo.gif"), false, false).1, ".gif");
        assert_eq!(split_output_base(Path::new("photo.png"), true, false).1, ".png");
        assert_eq!(split_output_base(Path::new("dir/noext"), true, false).1, ".png");
        assert_eq!(split_output_base(Path::new("dir/noext"), false, false).1, "");
        assert_eq!(adjusted_ext(Path::new("a.jpg"), false, false), "png");
    }

    #[test]
    fn output_base_strips_extension() {
        let (base, ext) = split_output_base(Path::new("a/b/photo.png"), false, false);
        assert_eq!(base, "a/b/photo");
        assert_eq!(ext, ".png");
    }

    #[test]
    fn path_splitting() {
        assert_eq!(
            split_paths("a | b |c"),
            vec![PathBuf::from("a"), PathBuf::from("b"), PathBuf::from("c")]
        );
        assert!(split_paths("  |  ").is_empty());
    }

    #[test]
    fn toast_message_substitution() {
        assert_eq!(
            fill("Saved {0} in {1:.03f}s", "out.png", "1.234"),
            "Saved out.png in 1.234s"
        );
    }
}