//! Minimal, dependency-free INI reader/writer.
//!
//! It implements the subset of Python `configparser` behaviour that
//! realesrgan-gui relies on, so that the existing `config.ini` stays shared
//! between the Python and Rust builds:
//!
//! * `[section]` headers, `key = value` / `key : value` options
//! * option names are lower-cased, section names keep their case
//! * `#` / `;` comments and indented continuation lines
//! * `%%` is read as `%` and `%` is written as `%%` (BasicInterpolation escape)

use std::path::Path;

#[derive(Debug, Default, Clone)]
pub struct Ini {
    sections: Vec<(String, Vec<(String, String)>)>,
}

fn unescape(v: &str) -> String {
    v.replace("%%", "%")
}

fn escape(v: &str) -> String {
    v.replace('%', "%%")
}

impl Ini {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(s) => Self::parse(&s),
            Err(_) => Self::new(),
        }
    }

    pub fn parse(text: &str) -> Self {
        let mut ini = Self::new();
        let mut current: Option<usize> = None;
        let mut last_key: Option<usize> = None;
        for raw in text.lines() {
            let no_eol = raw.trim_end_matches(['\r', '\n']);
            if no_eol.trim().is_empty() {
                continue;
            }
            let trimmed = no_eol.trim();
            // Indented line -> continuation of the previous option.
            if no_eol.starts_with(' ') || no_eol.starts_with('\t') {
                if let (Some(sec), Some(key)) = (current, last_key) {
                    if let Some(opt) = ini.sections[sec].1.get_mut(key) {
                        opt.1.push('\n');
                        opt.1.push_str(trimmed);
                    }
                }
                continue;
            }
            last_key = None;
            if trimmed.starts_with('#') || trimmed.starts_with(';') {
                continue;
            }
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let name = trimmed[1..trimmed.len() - 1].trim().to_string();
                current = Some(ini.ensure(&name));
                continue;
            }
            let split = trimmed
                .find(['=', ':'])
                .map(|i| (i, &trimmed[i..i + 1]));
            if let Some((i, _)) = split {
                let key = trimmed[..i].trim().to_lowercase();
                let value = trimmed[i + 1..].trim().to_string();
                if let Some(sec) = current {
                    let list = &mut ini.sections[sec].1;
                    if let Some(pos) = list.iter().position(|(k, _)| *k == key) {
                        list[pos].1 = value;
                        last_key = Some(pos);
                    } else {
                        list.push((key, value));
                        last_key = Some(list.len() - 1);
                    }
                }
            }
        }
        ini
    }

    fn ensure(&mut self, section: &str) -> usize {
        if let Some(i) = self.sections.iter().position(|(n, _)| n == section) {
            return i;
        }
        self.sections.push((section.to_string(), Vec::new()));
        self.sections.len() - 1
    }

    /// All sections in file order.
    pub fn sections(&self) -> &[(String, Vec<(String, String)>)] {
        &self.sections
    }

    pub fn section(&self, section: &str) -> Option<&Vec<(String, String)>> {
        self.sections
            .iter()
            .find(|(n, _)| n == section)
            .map(|(_, v)| v)
    }

    pub fn get(&self, section: &str, key: &str) -> Option<String> {
        let k = key.to_lowercase();
        self.section(section)?
            .iter()
            .find(|(a, _)| *a == k)
            .map(|(_, v)| unescape(v))
    }

    pub fn get_bool(&self, section: &str, key: &str) -> Option<bool> {
        self.get(section, key).map(|v| is_truthy(&v))
    }

    pub fn get_i64(&self, section: &str, key: &str) -> Option<i64> {
        self.get(section, key).and_then(|v| v.trim().parse().ok())
    }

    pub fn set(&mut self, section: &str, key: &str, value: impl AsRef<str>) {
        let k = key.to_lowercase();
        let v = escape(value.as_ref());
        let sec = self.ensure(section);
        let list = &mut self.sections[sec].1;
        if let Some(pos) = list.iter().position(|(a, _)| *a == k) {
            list[pos].1 = v;
        } else {
            list.push((k, v));
        }
    }

    pub fn clear_section(&mut self, section: &str) {
        self.sections.retain(|(n, _)| n != section);
    }

    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        let mut out = String::new();
        for (name, options) in &self.sections {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push('[');
            out.push_str(name);
            out.push_str("]\n");
            for (k, v) in options {
                out.push_str(k);
                out.push_str(" = ");
                out.push_str(v);
                out.push('\n');
            }
        }
        std::fs::write(path, out)
    }
}

pub fn is_truthy(v: &str) -> bool {
    matches!(
        v.trim().to_lowercase().as_str(),
        "1" | "yes" | "true" | "on"
    )
}

pub fn bool_str(v: bool) -> &'static str {
    if v {
        "True"
    } else {
        "False"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_lookup_is_case_insensitive() {
        let ini = Ini::parse("[Config]\nUpscaler = C:/x.exe\nResizeRatio : 8\n# comment\n; other\n");
        assert_eq!(ini.get("Config", "upscaler").as_deref(), Some("C:/x.exe"));
        assert_eq!(ini.get("Config", "RESIZERATIO").as_deref(), Some("8"));
        assert_eq!(ini.get_i64("Config", "resizeratio"), Some(8));
        assert!(ini.get("Config", "missing").is_none());
    }

    #[test]
    fn indented_lines_continue_previous_value() {
        let ini = Ini::parse("[A]\nkey = first\n    second\n  third\n");
        assert_eq!(ini.get("A", "key").as_deref(), Some("first\nsecond\nthird"));
    }

    #[test]
    fn percent_is_escaped_on_write_and_unescaped_on_read() {
        let mut ini = Ini::new();
        ini.set("Config", "CustomCommand", "echo 100%");
        let text = {
            let mut s = String::new();
            for (name, opts) in ini.sections() {
                s.push_str(&format!("[{name}]\n"));
                for (k, v) in opts {
                    s.push_str(&format!("{k} = {v}\n"));
                }
            }
            s
        };
        assert!(text.contains("100%%"));
        let reparsed = Ini::parse(&text);
        assert_eq!(reparsed.get("Config", "customcommand").as_deref(), Some("echo 100%"));
    }

    #[test]
    fn truthiness_matches_python_configparser() {
        for t in ["1", "yes", "True", "on", " ON "] {
            assert!(is_truthy(t), "{t} should be truthy");
        }
        for f in ["0", "no", "False", "off", ""] {
            assert!(!is_truthy(f), "{f} should be falsy");
        }
    }
}