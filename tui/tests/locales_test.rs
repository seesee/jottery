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
