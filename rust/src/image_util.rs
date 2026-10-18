//! Image loading, resizing and encoding helpers.

use std::path::Path;

use image::{DynamicImage, ImageEncoder, ImageFormat};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Resampling filters used for pre/post resizing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    Nearest,
    Triangle,
    CatmullRom,
    Gaussian,
    Lanczos3,
}

impl Filter {
    pub fn to_image(self) -> image::imageops::FilterType {
        match self {
            Self::Nearest => image::imageops::FilterType::Nearest,
            Self::Triangle => image::imageops::FilterType::Triangle,
            Self::CatmullRom => image::imageops::FilterType::CatmullRom,
            Self::Gaussian => image::imageops::FilterType::Gaussian,
            Self::Lanczos3 => image::imageops::FilterType::Lanczos3,
        }
    }
}

pub fn dimensions(path: &Path) -> Result<(u32, u32)> {
    Ok(image::image_dimensions(path)?)
}

pub fn load(path: &Path) -> Result<DynamicImage> {
    Ok(image::open(path)?)
}

/// Resize `img` to exactly `w` x `h` using `filter`.
pub fn resize(img: &DynamicImage, w: u32, h: u32, filter: Filter) -> DynamicImage {
    img.resize_exact(w.max(1), h.max(1), filter.to_image())
}

pub fn has_alpha(img: &DynamicImage) -> bool {
    use image::ColorType::*;
    matches!(img.color(), La8 | La16 | Rgba8 | Rgba16 | Rgba32F)
}

pub fn is_webp(path: &Path) -> bool {
    ext_is(path, &["webp"])
}

pub fn ext_is(path: &Path, exts: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| exts.iter().any(|x| x.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

/// Losslessly encode `img`, choosing the format from `path`'s extension.
pub fn save_lossless(img: &DynamicImage, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    if is_webp(path) {
        return save_webp_lossless(img, path);
    }
    if ext_is(path, &["jpg", "jpeg"]) {
        return save_jpeg(img, path, 100);
    }
    img.save(path)?;
    Ok(())
}

pub fn save_jpeg(img: &DynamicImage, path: &Path, quality: u8) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let rgb = img.to_rgb8();
    let mut file = std::fs::File::create(path)?;
    let mut encoder =
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, quality.min(100).max(1));
    encoder.encode(
        rgb.as_raw(),
        rgb.width(),
        rgb.height(),
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(())
}

pub fn save_webp_lossless(img: &DynamicImage, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let mut file = std::fs::File::create(path)?;
    let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut file);
    let rgba = img.to_rgba8();
    encoder.write_image(
        rgba.as_raw(),
        rgba.width(),
        rgba.height(),
        image::ExtendedColorType::Rgba8,
    )?;
    Ok(())
}

/// Lossy WebP encoding via libwebp.
pub fn save_webp_lossy(img: &DynamicImage, path: &Path, quality: f32) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let rgba = img.to_rgba8();
    let encoder = webp::Encoder::from_rgba(rgba.as_raw(), rgba.width(), rgba.height());
    let memory = encoder.encode(quality.clamp(0.0, 100.0));
    std::fs::write(path, &*memory)?;
    Ok(())
}

/// Encode an image for the "lossy" pipeline, choosing lossy WebP or JPEG.
pub fn save_lossy(img: &DynamicImage, path: &Path, quality: i64, warn_alpha: impl FnOnce()) -> Result<()> {
    if is_webp(path) {
        return save_webp_lossy(img, path, quality as f32);
    }
    if has_alpha(img) {
        warn_alpha();
    }
    save_jpeg(img, path, quality as u8)
}

/// Detect the output image format from a path, defaulting to PNG.
#[allow(dead_code)]
pub fn format_of(path: &Path) -> ImageFormat {
    path.extension()
        .and_then(|e| e.to_str())
        .and_then(|e| ImageFormat::from_extension(e))
        .unwrap_or(ImageFormat::Png)
}

/// Guess whether `path` is an image the app can process.
pub const SUPPORTED_EXTS: [&str; 7] = ["jpg", "jpeg", "png", "gif", "webp", "tif", "tiff"];

pub fn is_supported_image(path: &Path) -> bool {
    ext_is(path, &SUPPORTED_EXTS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, Rgba, RgbaImage};

    fn sample() -> DynamicImage {
        let mut img = RgbaImage::new(8, 4);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = Rgba([(x * 20) as u8, (y * 40) as u8, 128, 255]);
        }
        DynamicImage::ImageRgba8(img)
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("regui-img-{}-{name}", std::process::id()))
    }

    #[test]
    fn lossy_webp_round_trips_dimensions() {
        let img = sample();
        let path = temp_path("lossy.webp");
        save_webp_lossy(&img, &path, 75.0).unwrap();
        assert_eq!(dimensions(&path).unwrap(), (8, 4));
        // The bytes on disk must start with the RIFF/WEBP container magic.
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WEBP");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn lossless_webp_and_jpeg_write_readable_files() {
        let img = sample();
        let webp = temp_path("lossless.webp");
        save_webp_lossless(&img, &webp).unwrap();
        assert_eq!(dimensions(&webp).unwrap(), (8, 4));

        let jpg = temp_path("photo.jpg");
        save_jpeg(&img, &jpg, 90).unwrap();
        assert_eq!(dimensions(&jpg).unwrap(), (8, 4));

        std::fs::remove_file(&webp).ok();
        std::fs::remove_file(&jpg).ok();
    }

    #[test]
    fn extension_helpers() {
        assert!(is_supported_image(Path::new("a/b/PHOTO.JPEG")));
        assert!(is_webp(Path::new("x.WebP")));
        assert!(!is_supported_image(Path::new("notes.txt")));
        assert!(!is_supported_image(Path::new("noext")));
    }

    #[test]
    fn resize_is_exact() {
        let out = resize(&sample(), 16, 16, Filter::Lanczos3);
        assert_eq!(out.dimensions(), (16, 16));
        // Never collapses to zero pixels.
        let tiny = resize(&sample(), 0, 0, Filter::Nearest);
        assert_eq!(tiny.dimensions(), (1, 1));
    }
}