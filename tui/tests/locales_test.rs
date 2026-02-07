//! Translation Coverage Tests for TUI
//!
//! Verifies that all locale YAML files have the same keys as the primary locale (en-GB).
//! This ensures no translation keys are missing in any language.

use serde_yaml::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

const PRIMARY_LOCALE: &str = "en-GB";

fn get_locales_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("locales")
}

fn get_locale_files() -> Vec<String> {
    let locales_dir = get_locales_dir();
    fs::read_dir(&locales_dir)
        .expect("Failed to read locales directory")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension()?.to_str()? == "yml" {
                path.file_stem()?.to_str().map(String::from)
            } else {
                None
            }
        })
        .collect()
}

fn load_locale(name: &str) -> Value {
    let path = get_locales_dir().join(format!("{}.yml", name));
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read locale file {}: {}", name, e));
    serde_yaml::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse locale file {}: {}", name, e))
}

fn extract_keys(value: &Value, prefix: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();

    if let Value::Mapping(map) = value {
        for (k, v) in map {
            if let Value::String(key) = k {
                let full_key = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", prefix, key)
                };

                if let Value::Mapping(_) = v {
                    keys.extend(extract_keys(v, &full_key));
                } else {
                    keys.insert(full_key);
                }
            }
        }
    }

    keys
}

#[test]
fn test_primary_locale_exists() {
    let locales = get_locale_files();
    assert!(
        locales.contains(&PRIMARY_LOCALE.to_string()),
        "Primary locale {} not found",
        PRIMARY_LOCALE
    );
}

#[test]
fn test_has_minimum_locale_count() {
    let locales = get_locale_files();
    assert!(
        locales.len() >= 10,
        "Expected at least 10 locale files, found {}",
        locales.len()
    );
}

#[test]
fn test_primary_locale_has_keys() {
    let primary = load_locale(PRIMARY_LOCALE);
    let keys = extract_keys(&primary, "");
    assert!(
        keys.len() > 50,
        "Primary locale should have more than 50 keys, found {}",
        keys.len()
    );
}

#[test]
fn test_all_locales_have_required_keys() {
    let primary = load_locale(PRIMARY_LOCALE);
    let primary_keys = extract_keys(&primary, "");

    let locales = get_locale_files();
    let mut failures = Vec::new();

    for locale_name in locales {
        if locale_name == PRIMARY_LOCALE {
            continue;
        }

        let locale = load_locale(&locale_name);
        let locale_keys = extract_keys(&locale, "");

        let missing: Vec<_> = primary_keys
            .iter()
            .filter(|k| !locale_keys.contains(*k))
            .collect();

        if !missing.is_empty() {
            let missing_preview: Vec<_> = missing.iter().take(10).map(|s| s.as_str()).collect();
            failures.push(format!(
                "{}: missing {} keys (first 10: {})",
                locale_name,
                missing.len(),
                missing_preview.join(", ")
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "Translation coverage failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn test_locale_key_structure() {
    let primary = load_locale(PRIMARY_LOCALE);

    // Verify expected top-level sections exist
    let expected_sections = ["app", "common", "password", "menu", "note", "settings", "sync"];

    for section in &expected_sections {
        assert!(
            primary
                .as_mapping()
                .map(|m| m.contains_key(&Value::String(section.to_string())))
                .unwrap_or(false),
            "Primary locale should have '{}' section",
            section
        );
    }
}

#[test]
fn test_no_empty_translations() {
    let locales = get_locale_files();
    let mut failures = Vec::new();

    for locale_name in locales {
        let locale = load_locale(&locale_name);
        let empty_keys = find_empty_values(&locale, "");

        if !empty_keys.is_empty() {
            failures.push(format!(
                "{}: has {} empty translation values (first 5: {})",
                locale_name,
                empty_keys.len(),
                empty_keys.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "Empty translation values found:\n{}",
        failures.join("\n")
    );
}

fn find_empty_values(value: &Value, prefix: &str) -> Vec<String> {
    let mut empty = Vec::new();

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
                        empty.extend(find_empty_values(v, &full_key));
                    }
                    Value::String(s) if s.trim().is_empty() => {
                        empty.push(full_key);
                    }
                    _ => {}
                }
            }
        }
    }

    empty
}

/// Extract all key-value pairs as a flat map
fn extract_key_values(value: &Value, prefix: &str) -> std::collections::HashMap<String, String> {
    let mut result = std::collections::HashMap::new();

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

/// Keys that are expected to have the same value across all locales
/// (product names, technical terms, keyboard shortcuts, etc.)
fn is_excluded_key(key: &str) -> bool {
    let excluded_keys = [
        "app.name",
        "editor.default_editor",
        "common.email",           // "Email" is the same in many languages
        "settings.disabled",      // "disabled" shown as placeholder
        "backup.records",         // "records" is technical
        "time.date_format",       // Date format patterns
        "time.time_format",
        "time.datetime_format",
    ];

    let excluded_patterns = [
        "syntax.",       // Programming language names
        "color_scheme.", // Theme names
    ];

    // Keyboard shortcut keys (e.g., help.ctrl_q_key, help.slash_search_key)
    let excluded_suffixes = [
        "_key",          // All keyboard shortcut key labels
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

/// Values that are intentionally kept as English across all locales
fn is_excluded_value(value: &str) -> bool {
    let excluded_values = [
        "Jottery",
        "OK",
        "vi",
        "Email",
        "disabled",
        "records",
        "minutes",
        "base64",
        // Single words that might be the same in some languages
        "Archived",
        "Error",
        "Note",
        // Programming terms
        "JavaScript", "Python", "Markdown", "JSON", "HTML", "CSS",
        "SQL", "Bash", "Perl", "Rust", "Go", "Java", "C", "C++",
        // Theme names
        "Monokai", "Nord", "Dracula", "Catppuccin",
        // Keyboard shortcuts and vim-style commands
        "Ctrl+Q", "Ctrl+R", "Ctrl+A", "Ctrl+d/u/f/b",
        "Tab", "Enter", "Esc", "Space", "Backspace",
        "PgUp", "PgDn", "Shift+V", "Shift+A",
    ];

    // Very short values (3 chars or less) are often the same
    if value.len() <= 3 {
        return true;
    }

    // Check if value matches excluded list
    if excluded_values.contains(&value) {
        return true;
    }

    // Values that are purely format strings like "%{count}"
    if value.starts_with("%{") && value.ends_with("}") {
        return true;
    }

    // URLs or technical patterns
    if value.starts_with("http") || value.contains("://") {
        return true;
    }

    // Keyboard shortcut patterns (e.g., "j / k", "↑ / ↓", "Shift+j / Shift+k")
    if value.contains(" / ") && value.len() < 30 {
        return true;
    }

    // Vim-style command references
    if value.starts_with("$EDITOR") || value.contains("$EDITOR") {
        return true;
    }

    // Color search patterns (e.g., "color:red note")
    if value.starts_with("color:") {
        return true;
    }

    // Tag search patterns (e.g., "#tag")
    if value.starts_with("#") && !value.contains(" ") {
        return true;
    }

    false
}

/// Test translation quality - warns about potentially untranslated strings
/// This test logs warnings for untranslated strings but only fails if more than
/// 50% of strings in a locale are untranslated (indicating a major issue).
#[test]
fn test_translation_quality() {
    let primary = load_locale(PRIMARY_LOCALE);
    let primary_values = extract_key_values(&primary, "");
    let total_translatable = primary_values
        .iter()
        .filter(|(k, v)| !is_excluded_key(k) && !is_excluded_value(v))
        .count();

    let locales = get_locale_files();
    let mut warnings = Vec::new();
    let mut critical_failures = Vec::new();

    for locale_name in locales {
        // Skip primary locale and en-US (intentionally similar to en-GB)
        if locale_name == PRIMARY_LOCALE || locale_name == "en-US" {
            continue;
        }

        let locale = load_locale(&locale_name);
        let locale_values = extract_key_values(&locale, "");

        let mut untranslated = Vec::new();

        for (key, english_value) in &primary_values {
            // Skip excluded keys and values
            if is_excluded_key(key) || is_excluded_value(english_value) {
                continue;
            }

            if let Some(locale_value) = locale_values.get(key) {
                // Check if values are identical (potentially untranslated)
                if english_value == locale_value {
                    untranslated.push(format!("{}: \"{}\"", key,
                        if english_value.len() > 40 {
                            format!("{}...", &english_value[..40])
                        } else {
                            english_value.clone()
                        }
                    ));
                }
            }
        }

        if !untranslated.is_empty() {
            let percentage = (untranslated.len() as f64 / total_translatable as f64) * 100.0;

            let msg = format!(
                "{}: {} potentially untranslated strings ({:.1}%) (first 5):\n  - {}",
                locale_name,
                untranslated.len(),
                percentage,
                untranslated.iter().take(5).cloned().collect::<Vec<_>>().join("\n  - ")
            );

            // Fail if more than 50% untranslated (indicates major issue)
            if percentage > 50.0 {
                critical_failures.push(msg);
            } else {
                warnings.push(msg);
            }
        }
    }

    // Print warnings (these are tracked but don't fail the test)
    if !warnings.is_empty() {
        eprintln!("\n⚠️  Translation quality warnings (non-blocking):\n{}\n",
            warnings.join("\n\n"));
    }

    // Fail only on critical issues
    assert!(
        critical_failures.is_empty(),
        "Critical translation issues (>50% untranslated):\n{}",
        critical_failures.join("\n\n")
    );
}
