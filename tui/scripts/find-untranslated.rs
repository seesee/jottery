//! Script to find untranslated strings in TUI locale files
//! Run with: cargo run --bin find-untranslated -- de

use serde_yaml::Value;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

const PRIMARY_LOCALE: &str = "en-GB";

fn get_locales_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("locales")
}

fn load_locale(name: &str) -> Value {
    let path = get_locales_dir().join(format!("{}.yml", name));
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read locale file {}: {}", name, e));
    serde_yaml::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse locale file {}: {}", name, e))
}

fn extract_key_values(value: &Value, prefix: &str) -> HashMap<String, String> {
    let mut result = HashMap::new();

    if let Value::Mapping(map) = value {
        for (k, v) in map {
            if let Value::String(key) = k {
                let full_key = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", prefix, key)
                };

                match v {
                    Value::Mapping(_) => {
                        result.extend(extract_key_values(v, &full_key));
                    }
                    Value::String(s) => {
                        result.insert(full_key, s.clone());
                    }
                    _ => {}
                }
            }
        }
    }

    result
}

fn is_excluded_key(key: &str) -> bool {
    let excluded_keys = [
        "app.name",
        "editor.default_editor",
        "common.email",
        "settings.disabled",
        "backup.records",
        "time.date_format",
        "time.time_format",
        "time.datetime_format",
    ];

    let excluded_patterns = [
        "syntax.",
        "color_scheme.",
    ];

    let excluded_suffixes = [
        "_key",
    ];

    if excluded_keys.contains(&key) {
        return true;
    }

    for pattern in &excluded_patterns {
        if key.starts_with(pattern) {
            return true;
        }
    }

    for suffix in &excluded_suffixes {
        if key.ends_with(suffix) {
            return true;
        }
    }

    false
}

fn is_excluded_value(value: &str) -> bool {
    let excluded_values = [
        "Jottery", "OK", "vi", "Email", "disabled", "records", "minutes", "base64",
        "Archived", "Error", "Note",
        "JavaScript", "Python", "Markdown", "JSON", "HTML", "CSS",
        "SQL", "Bash", "Perl", "Rust", "Go", "Java", "C", "C++",
        "Monokai", "Nord", "Dracula", "Catppuccin",
        "Ctrl+Q", "Ctrl+R", "Ctrl+A", "Ctrl+d/u/f/b",
        "Tab", "Enter", "Esc", "Space", "Backspace",
        "PgUp", "PgDn", "Shift+V", "Shift+A",
    ];

    if value.len() <= 3 {
        return true;
    }

    if excluded_values.contains(&value) {
        return true;
    }

    if value.starts_with("%{") && value.ends_with("}") {
        return true;
    }

    if value.starts_with("http") || value.contains("://") {
        return true;
    }

    if value.contains(" / ") && value.len() < 30 {
        return true;
    }

    if value.contains("$EDITOR") {
        return true;
    }

    if value.starts_with("color:") {
        return true;
    }

    if value.starts_with("#") && !value.contains(" ") {
        return true;
    }

    false
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let target_locale = args.get(1).map(|s| s.as_str()).unwrap_or("de");

    let primary = load_locale(PRIMARY_LOCALE);
    let primary_values = extract_key_values(&primary, "");

    let target = load_locale(target_locale);
    let target_values = extract_key_values(&target, "");

    println!("# Untranslated strings in {} (compared to {})", target_locale, PRIMARY_LOCALE);
    println!("# Format: key: \"english_value\"");
    println!();

    let mut untranslated: Vec<_> = primary_values
        .iter()
        .filter(|(key, english_value)| {
            if is_excluded_key(key) || is_excluded_value(english_value) {
                return false;
            }
            if let Some(target_value) = target_values.get(*key) {
                target_value == *english_value
            } else {
                true // Missing key
            }
        })
        .collect();

    untranslated.sort_by(|a, b| a.0.cmp(b.0));

    for (key, value) in untranslated {
        println!("{}: \"{}\"", key, value.replace("\"", "\\\""));
    }
}
