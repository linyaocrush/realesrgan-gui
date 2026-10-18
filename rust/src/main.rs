#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod define;
mod i18n;
mod image_util;
mod ini;
mod models;
mod params;
mod task;
mod theme;
mod win;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    win::com_init();

    // Ship a copy of the translations next to the executable so users can
    // customise them; the embedded copy is used when the file is missing.
    let i18n_path = define::i18n_path();
    if !i18n_path.is_file() {
        if let Err(e) = std::fs::write(&i18n_path, i18n::embedded_text()) {
            log::warn!("failed to write {}: {e}", i18n_path.display());
        }
    }

    let initial_inputs: Vec<PathBuf> = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect();

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: egui::ViewportBuilder::default()
            .with_title(define::APP_TITLE)
            .with_inner_size([720.0, 640.0])
            .with_min_inner_size([560.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        define::APP_TITLE,
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, initial_inputs)))),
    )
}