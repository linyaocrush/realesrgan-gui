//! Resize modes, downsampling filters and the per-run parameter bundle.

use std::path::PathBuf;

use crate::image_util::Filter;
use crate::ini::is_truthy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeMode {
    Ratio,
    Width,
    Height,
    LongestSide,
    ShortestSide,
}

impl ResizeMode {
    pub fn from_i64(v: i64) -> Self {
        match v {
            2 => Self::Width,
            3 => Self::Height,
            4 => Self::LongestSide,
            5 => Self::ShortestSide,
            _ => Self::Ratio,
        }
    }

    /// Suffix used in generated output file names (`x4`, `w1024`, ...).
    pub fn suffix(self, value: i64) -> String {
        match self {
            Self::Ratio => format!("x{value}"),
            Self::Width => format!("w{value}"),
            Self::Height => format!("h{value}"),
            Self::LongestSide => format!("l{value}"),
            Self::ShortestSide => format!("s{value}"),
        }
    }
}

/// Downsampling filters, mirroring Pillow's `Image.Resampling` names.
///
/// `image` has no exact equivalents for `Box` and `Hamming`; the closest
/// filters are used instead (`Box` -> `Triangle`, `Hamming` -> `Gaussian`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Downsample {
    Lanczos,
    Bicubic,
    Hamming,
    Bilinear,
    Box,
    Nearest,
}

impl Downsample {
    pub const ALL: [(Downsample, &'static str); 6] = [
        (Downsample::Lanczos, "Lanczos"),
        (Downsample::Bicubic, "Bicubic"),
        (Downsample::Hamming, "Hamming"),
        (Downsample::Bilinear, "Bilinear"),
        (Downsample::Box, "Box"),
        (Downsample::Nearest, "Nearest"),
    ];

    pub fn from_index(i: usize) -> Self {
        Self::ALL.get(i).map(|(d, _)| *d).unwrap_or(Downsample::Lanczos)
    }

    pub fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(d, _)| *d == self)
            .map(|(_, n)| *n)
            .unwrap_or("Lanczos")
    }

    pub fn filter(self) -> Filter {
        match self {
            Self::Lanczos => Filter::Lanczos3,
            Self::Bicubic => Filter::CatmullRom,
            Self::Hamming => Filter::Gaussian,
            Self::Bilinear => Filter::Triangle,
            Self::Box => Filter::Triangle,
            Self::Nearest => Filter::Nearest,
        }
    }
}

/// Tile sizes offered in the UI; index 0 means "auto" (`0` on the command line).
pub const TILE_SIZES: [i64; 9] = [0, 32, 64, 128, 256, 512, 1024, 2048, 4096];

/// Everything needed to run one upscale job.
#[derive(Debug, Clone)]
pub struct ReConfigParams {
    pub model: String,
    pub model_factor: i64,
    pub model_dir: PathBuf,
    pub resize_mode: ResizeMode,
    pub resize_mode_value: i64,
    pub downsample: Downsample,
    pub tile_size: i64,
    pub gpu_id: i64,
    pub use_tta: bool,
    pub preupscale: bool,
}

/// Parse the model pack factor from a model name, e.g. `realesrgan-x4plus`
/// or `4x-UltraSharp` -> `4`. Defaults to 4.
pub fn model_factor(name: &str) -> i64 {
    let bytes = name.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let after = if name[i..].starts_with('x') || name[i..].starts_with('X') {
                Some(&name[i..])
            } else {
                None
            };
            let before = start > 0 && (name[..start].ends_with('x') || name[..start].ends_with('X'));
            if after.is_some() || before {
                if let Ok(v) = name[start..i].parse::<i64>() {
                    if v > 0 {
                        return v;
                    }
                }
            }
            continue;
        }
        i += 1;
    }
    4
}

#[allow(dead_code)]
pub fn truthy(v: &str) -> bool {
    is_truthy(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_factor_reads_nx_or_xn() {
        assert_eq!(model_factor("realesrgan-x4plus"), 4);
        assert_eq!(model_factor("realesrgan-x4plus-anime"), 4);
        assert_eq!(model_factor("4x-UltraSharp"), 4);
        assert_eq!(model_factor("2x-Anime"), 2);
        assert_eq!(model_factor("no-factor-here"), 4);
        assert_eq!(model_factor("empty-x"), 4);
    }

    #[test]
    fn resize_mode_round_trips_and_names_suffixes() {
        assert_eq!(ResizeMode::from_i64(1), ResizeMode::Ratio);
        assert_eq!(ResizeMode::from_i64(2), ResizeMode::Width);
        assert_eq!(ResizeMode::from_i64(3), ResizeMode::Height);
        assert_eq!(ResizeMode::from_i64(4), ResizeMode::LongestSide);
        assert_eq!(ResizeMode::from_i64(5), ResizeMode::ShortestSide);
        assert_eq!(ResizeMode::from_i64(99), ResizeMode::Ratio);
        assert_eq!(ResizeMode::from_i64(1).suffix(4), "x4");
        assert_eq!(ResizeMode::from_i64(2).suffix(1024), "w1024");
        assert_eq!(ResizeMode::from_i64(3).suffix(768), "h768");
        assert_eq!(ResizeMode::from_i64(4).suffix(2048), "l2048");
        assert_eq!(ResizeMode::from_i64(5).suffix(512), "s512");
    }

    #[test]
    fn downsample_lookup_falls_back_to_lanczos() {
        assert_eq!(Downsample::from_index(0).name(), "Lanczos");
        assert_eq!(Downsample::from_index(5).name(), "Nearest");
        assert_eq!(Downsample::from_index(99).name(), "Lanczos");
        assert_eq!(Downsample::Bicubic.filter(), Filter::CatmullRom);
        assert_eq!(Downsample::Hamming.filter(), Filter::Gaussian);
        assert_eq!(Downsample::Box.filter(), Filter::Triangle);
    }
}