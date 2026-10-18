//! Translations and locale handling.
//!
//! The `i18n.ini` file is embedded at compile time for fast, dependency-free
//! startup; an `i18n.ini` next to the executable overrides it when present.

use std::collections::HashMap;
use std::path::Path;

use crate::define;
use crate::ini::Ini;

/// Embedded copy of the project's `i18n.ini`.
const EMBEDDED_I18N: &str = include_str!("../../i18n.ini");

/// Display name for every supported locale, in the order they appear in the UI.
const LOCALE_NAMES: [(&str, &str); 14] = [
    ("zh_CN", "中文 (简体, 中国)"),
    ("zh_SG", "中文 (简体, 新加坡)"),
    ("zh_HK", "中文 (繁體字, 中國香港特別行政區)"),
    ("zh_MO", "中文 (繁體字, 中國澳門特別行政區)"),
    ("zh_TW", "中文 (繁體, 台灣)"),
    ("en_US", "English (United States)"),
    ("en_GB", "English (United Kingdom)"),
    ("uk_UA", "українська (Україна)"),
    ("tr_TR", "Türkçe (Türkiye)"),
    ("es_ES", "español (España)"),
    ("es_MX", "español (México)"),
    ("es_CO", "español (Colombia)"),
    ("es_AR", "español (Argentina)"),
    ("fr_FR", "français (France)"),
];

pub struct I18n {
    /// locale code -> (key -> text)
    translations: HashMap<String, HashMap<String, String>>,
    /// Locale codes in display order.
    pub locales: Vec<String>,
    /// Display names, parallel to `locales`.
    pub locale_names: Vec<String>,
    pub current: String,
    /// System locale, used for the `key$locale` fallback.
    system_locale: String,
}

impl Default for I18n {
    fn default() -> Self {
        Self::new()
    }
}

impl I18n {
    pub fn new() -> Self {
        let path = define::i18n_path();
        let parsed = if path.is_file() {
            Ini::load(&path)
        } else {
            Ini::parse(EMBEDDED_I18N)
        };
        Self::from_ini(&parsed)
    }

    fn from_ini(ini: &Ini) -> Self {
        let mut translations: HashMap<String, HashMap<String, String>> = HashMap::new();
        for (section, options) in ini.sections() {
            let map: HashMap<String, String> =
                options.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            for lang in section.split(',') {
                let lang = lang.trim();
                if lang.is_empty() {
                    continue;
                }
                translations.insert(lang.to_string(), map.clone());
            }
        }

        // Keep the display order of `LOCALE_NAMES`, only for locales we have.
        let mut locales = Vec::new();
        let mut locale_names = Vec::new();
        for (code, name) in LOCALE_NAMES {
            if translations.contains_key(code) {
                locales.push(code.to_string());
                locale_names.push(name.to_string());
            }
        }

        let system_locale = sys_locale::get_locale().unwrap_or_else(|| "en_US".to_string());
        Self {
            translations,
            locales,
            locale_names,
            current: "en_US".to_string(),
            system_locale,
        }
    }

    pub fn has_locale(&self, code: &str) -> bool {
        self.locales.iter().any(|c| c == code)
    }

    /// Choose `preferred` when known, else the system locale, else English.
    pub fn set_language(&mut self, preferred: &str) {
        let preferred = preferred.trim();
        if self.has_locale(preferred) {
            self.current = preferred.to_string();
        } else if self.has_locale(&self.system_locale) {
            self.current = self.system_locale.clone();
        } else {
            self.current = "en_US".to_string();
        }
    }

    /// Index of the current locale in `locales`, for the language combo box.
    pub fn current_index(&self) -> usize {
        self.locales
            .iter()
            .position(|c| *c == self.current)
            .unwrap_or(0)
    }

    pub fn translated(&self, key: &str) -> Option<String> {
        // `configparser` lower-cases option names, and the Python app relied on
        // that case-insensitive lookup, so match it here.
        let key = key.to_lowercase();
        self.translations
            .get(&self.current)
            .and_then(|m| m.get(key.as_str()))
            .or_else(|| self.translations.get("en_US").and_then(|m| m.get(key.as_str())))
            .filter(|s| !s.is_empty())
            .cloned()
    }

    /// Translate `key`, falling back to `key$<system locale>` like the Python
    /// version did for untranslated strings.
    pub fn tr(&self, key: &str) -> String {
        self.translated(key)
            .unwrap_or_else(|| format!("{key}${}", self.system_locale))
    }

    pub fn system_locale(&self) -> &str {
        &self.system_locale
    }
}

/// Read `i18n.ini` from an arbitrary path (used by tests/tools).
#[allow(dead_code)]
pub fn load_from(path: &Path) -> I18n {
    I18n::from_ini(&Ini::load(path))
}

/// Expose the embedded text so it can be written out next to the executable.
pub fn embedded_text() -> &'static str {
    EMBEDDED_I18N
}

#[cfg(test)]
mod tests {
    use super::*;

    fn embedded() -> I18n {
        I18n::from_ini(&Ini::parse(EMBEDDED_I18N))
    }

    #[test]
    fn embedded_ini_provides_all_locales() {
        let i18n = embedded();
        assert_eq!(i18n.locales.len(), 14);
        assert_eq!(i18n.locale_names.len(), 14);
        assert!(i18n.locales.contains(&"en_US".to_string()));
        assert!(i18n.locales.contains(&"zh_CN".to_string()));
        assert!(i18n.locales.contains(&"fr_FR".to_string()));
        // Display order follows LOCALE_NAMES.
        assert_eq!(i18n.locales[0], "zh_CN");
        assert_eq!(i18n.locales[13], "fr_FR");
    }

    #[test]
    fn known_keys_translate_without_fallback_marker() {
        let mut i18n = embedded();
        i18n.set_language("en_US");
        for key in ["StartProcessing", "Input", "Output", "FrameBasicConfig"] {
            let t = i18n.tr(key);
            assert!(!t.is_empty(), "empty translation for {key}");
            assert!(!t.contains('$'), "fallback marker leaked for {key}: {t}");
        }
    }

    #[test]
    fn unknown_key_uses_python_style_fallback() {
        let i18n = embedded();
        assert!(i18n.tr("NopeNotAKey").starts_with("NopeNotAKey$"));
    }

    #[test]
    fn unknown_language_falls_back_to_a_known_locale() {
        let mut i18n = embedded();
        i18n.set_language("xx_XX");
        assert!(i18n.has_locale(&i18n.current), "current={}", i18n.current);
    }
}