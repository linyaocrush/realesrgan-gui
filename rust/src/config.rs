//! Application configuration, persisted to `config.ini`.
//!
//! The section name and option names match the Python version exactly so that
//! both builds can share the same `config.ini` file.

use std::path::Path;

use crate::i18n::I18n;
use crate::ini::{bool_str, Ini};

pub const SECTION: &str = "Config";

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub upscaler: String,
    pub model_dir: String,
    pub resize_mode: i64,
    pub resize_ratio: i64,
    pub resize_width: i64,
    pub resize_height: i64,
    pub resize_longest_side: i64,
    pub resize_shortest_side: i64,
    pub model: String,
    pub downsample_index: usize,
    pub gpu_id: i64,
    pub tile_size_index: usize,
    pub lossy_quality: i64,
    pub use_webp: bool,
    pub use_tta: bool,
    pub optimize_gif: bool,
    pub lossy_mode: bool,
    pub ignore_error: bool,
    pub preupscale: bool,
    pub custom_command: String,
    pub app_language: String,
}

impl AppConfig {
    pub fn defaults(i18n: &I18n) -> Self {
        Self {
            upscaler: String::new(),
            model_dir: String::new(),
            resize_mode: 1,
            resize_ratio: 4,
            resize_width: 1024,
            resize_height: 1024,
            resize_longest_side: 1024,
            resize_shortest_side: 1024,
            model: String::new(),
            downsample_index: 0,
            gpu_id: -1,
            tile_size_index: 0,
            lossy_quality: 80,
            use_webp: false,
            use_tta: false,
            optimize_gif: false,
            lossy_mode: false,
            ignore_error: false,
            preupscale: false,
            custom_command: String::new(),
            app_language: i18n.system_locale().to_string(),
        }
    }

    pub fn load(path: &Path, i18n: &I18n) -> Self {
        let ini = Ini::load(path);
        let mut cfg = Self::defaults(i18n);
        let get = |k: &str| ini.get(SECTION, k);

        if let Some(v) = get("upscaler") {
            cfg.upscaler = v;
        }
        if let Some(v) = get("modeldir") {
            cfg.model_dir = v;
        }
        if let Some(v) = ini.get_i64(SECTION, "resizemode") {
            cfg.resize_mode = v;
        }
        if let Some(v) = ini.get_i64(SECTION, "resizeratio") {
            cfg.resize_ratio = v;
        }
        if let Some(v) = ini.get_i64(SECTION, "resizewidth") {
            cfg.resize_width = v;
        }
        if let Some(v) = ini.get_i64(SECTION, "resizeheight") {
            cfg.resize_height = v;
        }
        if let Some(v) = ini.get_i64(SECTION, "resizelongestside") {
            cfg.resize_longest_side = v;
        }
        if let Some(v) = ini.get_i64(SECTION, "resizeshortestside") {
            cfg.resize_shortest_side = v;
        }
        if let Some(v) = get("model") {
            cfg.model = v;
        }
        if let Some(v) = ini.get_i64(SECTION, "downsampleindex") {
            cfg.downsample_index = v.max(0) as usize;
        }
        if let Some(v) = ini.get_i64(SECTION, "gpuid") {
            cfg.gpu_id = v;
        }
        if let Some(v) = ini.get_i64(SECTION, "tilesizeindex") {
            cfg.tile_size_index = v.max(0) as usize;
        }
        if let Some(v) = ini.get_i64(SECTION, "lossyquality") {
            cfg.lossy_quality = v;
        }
        if let Some(v) = ini.get_bool(SECTION, "usewebp") {
            cfg.use_webp = v;
        }
        if let Some(v) = ini.get_bool(SECTION, "usetta") {
            cfg.use_tta = v;
        }
        if let Some(v) = ini.get_bool(SECTION, "optimizegif") {
            cfg.optimize_gif = v;
        }
        if let Some(v) = ini.get_bool(SECTION, "lossymode") {
            cfg.lossy_mode = v;
        }
        if let Some(v) = ini.get_bool(SECTION, "ignoreerror") {
            cfg.ignore_error = v;
        }
        if let Some(v) = ini.get_bool(SECTION, "preupscale") {
            cfg.preupscale = v;
        }
        if let Some(v) = get("customcommand") {
            cfg.custom_command = v;
        }
        if let Some(v) = get("applanguage") {
            cfg.app_language = v;
        }
        cfg
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let mut ini = Ini::load(path);
        ini.clear_section(SECTION);
        ini.set(SECTION, "Upscaler", &self.upscaler);
        ini.set(SECTION, "ModelDir", &self.model_dir);
        ini.set(SECTION, "ResizeMode", self.resize_mode.to_string());
        ini.set(SECTION, "ResizeRatio", self.resize_ratio.to_string());
        ini.set(SECTION, "ResizeWidth", self.resize_width.to_string());
        ini.set(SECTION, "ResizeHeight", self.resize_height.to_string());
        ini.set(SECTION, "ResizeLongestSide", self.resize_longest_side.to_string());
        ini.set(SECTION, "ResizeShortestSide", self.resize_shortest_side.to_string());
        ini.set(SECTION, "Model", &self.model);
        ini.set(SECTION, "DownsampleIndex", self.downsample_index.to_string());
        ini.set(SECTION, "GPUID", self.gpu_id.to_string());
        ini.set(SECTION, "TileSizeIndex", self.tile_size_index.to_string());
        ini.set(SECTION, "LossyQuality", self.lossy_quality.to_string());
        ini.set(SECTION, "UseWebP", bool_str(self.use_webp));
        ini.set(SECTION, "UseTTA", bool_str(self.use_tta));
        ini.set(SECTION, "OptimizeGIF", bool_str(self.optimize_gif));
        ini.set(SECTION, "LossyMode", bool_str(self.lossy_mode));
        ini.set(SECTION, "IgnoreError", bool_str(self.ignore_error));
        ini.set(SECTION, "Preupscale", bool_str(self.preupscale));
        ini.set(SECTION, "CustomCommand", &self.custom_command);
        ini.set(SECTION, "AppLanguage", &self.app_language);
        ini.write(path)
    }
}