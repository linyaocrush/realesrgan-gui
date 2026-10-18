//! Paths, titles and upscaler-executable discovery.

use std::path::{Path, PathBuf};

pub const APP_TITLE: &str = "Real-ESRGAN GUI";

/// Compile-time build timestamp, set by the build environment when available.
pub const BUILD_TIME: Option<&str> = option_env!("REGUI_BUILD_TIME");

/// Candidate upscaler executables, in the same order the Python version used.
const UPSCALER_NAMES: [&str; 3] = [
    "upscayl-bin",
    "realesrgan-ncnn-vulkan",
    "realcugan-ncnn-vulkan",
];

/// Directory containing the running executable.
pub fn app_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// The Python version had a separate `BASE_PATH` for frozen bundles; the Rust
/// build ships as a plain executable, so both point at the executable's dir.
pub fn base_path() -> PathBuf {
    app_path()
}

pub fn config_path() -> PathBuf {
    app_path().join("config.ini")
}

pub fn i18n_path() -> PathBuf {
    app_path().join("i18n.ini")
}

pub fn default_models_dir() -> PathBuf {
    app_path().join("models")
}

pub fn exe_suffix(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Resolve the upscaler executable: an explicit path from the config wins,
/// otherwise the known executable names next to the app are probed.
pub fn detect_re_path(custom: Option<&str>) -> Option<PathBuf> {
    if let Some(c) = custom {
        let c = c.trim();
        if !c.is_empty() {
            let p = PathBuf::from(c);
            if p.is_file() {
                return Some(dunce_canonicalize(&p));
            }
        }
    }
    for name in UPSCALER_NAMES {
        let p = app_path().join(exe_suffix(name));
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn is_realcugan(re_path: &Path) -> bool {
    re_path
        .file_stem()
        .map(|s| s.to_string_lossy() == "realcugan-ncnn-vulkan")
        .unwrap_or(false)
}

pub fn is_upscayl(re_path: &Path) -> bool {
    re_path
        .file_stem()
        .map(|s| s.to_string_lossy() == "upscayl-bin")
        .unwrap_or(false)
}

/// Canonicalise and strip the Windows `\\?\` verbatim prefix where possible.
pub fn dunce_canonicalize(p: &Path) -> PathBuf {
    let c = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    #[cfg(windows)]
    {
        if let Some(s) = c.to_str() {
            if let Some(stripped) = s.strip_prefix(r"\\?\") {
                return PathBuf::from(stripped);
            }
        }
    }
    c
}

pub fn releases_url() -> &'static str {
    "https://github.com/xinntao/Real-ESRGAN-ncnn-vulkan/releases"
}

pub fn source_url() -> &'static str {
    "https://github.com/linyaocrush/realesrgan-gui"
}

pub fn re_intro_url() -> &'static str {
    "https://github.com/xinntao/Real-ESRGAN-ncnn-vulkan"
}

pub fn additional_model_url() -> &'static str {
    "https://github.com/TransparentLC/realesrgan-gui/releases/tag/additional-models"
}

pub fn donate_url() -> &'static str {
    "https://i.akarin.dev/donate/"
}