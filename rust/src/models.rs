//! Discovery of available upscaling model packs.

use std::collections::BTreeSet;
use std::path::Path;

/// Noise-reduction variants exposed by Real-CUGAN.
const REALCUGAN_NOISES: [&str; 5] = [
    "conservative",
    "no-denoise",
    "denoise1x",
    "denoise2x",
    "denoise3x",
];

/// Enumerate models in `dir`.
///
/// For Real-CUGAN the models live in per-model sub-directories and are named
/// `name#up{scale}x-{noise}`; for Real-ESRGAN they are `{name}.bin` /
/// `{name}.param` pairs directly in `dir`.
pub fn list_models(dir: &Path, realcugan: bool) -> Vec<String> {
    if !dir.is_dir() {
        return Vec::new();
    }

    if realcugan {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        names.sort();

        let mut models = Vec::new();
        for name in names {
            for scale in 2..=4 {
                for noise in REALCUGAN_NOISES {
                    let base = dir.join(&name).join(format!("up{scale}x-{noise}"));
                    if base.with_extension("bin").is_file() && base.with_extension("param").is_file()
                    {
                        models.push(format!("{name}#up{scale}x-{noise}"));
                    }
                }
            }
        }
        models
    } else {
        let mut stems = BTreeSet::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    stems.insert(stem.to_string());
                }
            }
        }
        stems
            .into_iter()
            .filter(|s| {
                dir.join(format!("{s}.bin")).is_file() && dir.join(format!("{s}.param")).is_file()
            })
            .collect()
    }
}

/// Move the two bundled Real-ESRGAN models to the front, matching the Python
/// behaviour of showing `realesrgan-x4plus` first and `-anime` second.
pub fn reorder_preferred(models: &mut Vec<String>) {
    // Inserted front-to-back, so iterate in reverse: the last insert wins the
    // front slot.
    for name in ["realesrgan-x4plus-anime", "realesrgan-x4plus"] {
        if let Some(pos) = models.iter().position(|m| m == name) {
            let v = models.remove(pos);
            models.insert(0, v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("regui-models-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn lists_real_esrgan_pairs() {
        let dir = temp_dir("re");
        std::fs::write(dir.join("realesrgan-x4plus.bin"), b"").unwrap();
        std::fs::write(dir.join("realesrgan-x4plus.param"), b"").unwrap();
        std::fs::write(dir.join("lone.param"), b"").unwrap();
        let models = list_models(&dir, false);
        assert_eq!(models, vec!["realesrgan-x4plus".to_string()]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn lists_realcugan_combinations() {
        let dir = temp_dir("cugan");
        let sub = dir.join("2x-conservative");
        std::fs::create_dir_all(&sub).unwrap();
        for f in ["up2x-conservative.bin", "up2x-conservative.param"] {
            std::fs::write(sub.join(f), b"").unwrap();
        }
        // A lone .bin without .param must be ignored.
        std::fs::write(sub.join("up3x-denoise1x.bin"), b"").unwrap();
        let models = list_models(&dir, true);
        assert_eq!(models, vec!["2x-conservative#up2x-conservative".to_string()]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn preferred_models_go_first_in_python_order() {
        let mut models = vec![
            "a".to_string(),
            "realesrgan-x4plus-anime".to_string(),
            "b".to_string(),
            "realesrgan-x4plus".to_string(),
        ];
        reorder_preferred(&mut models);
        assert_eq!(
            models,
            vec![
                "realesrgan-x4plus".to_string(),
                "realesrgan-x4plus-anime".to_string(),
                "a".to_string(),
                "b".to_string(),
            ]
        );
    }
}