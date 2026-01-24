//! Note color utilities for rendering colored notes and tags in the TUI
//!
//! This module provides functions to convert hex colors from the user's color palette
//! to ratatui Color types for terminal rendering.

use ratatui::style::Color;
use crate::models::UserSettings;

/// Convert a hex color string to a ratatui Color
/// Supports formats: #RRGGBB or #RGB
/// Used for true color terminals, with ANSI fallback for limited terminals
pub fn hex_to_color(hex: &str) -> Option<Color> {
    let hex = hex.trim_start_matches('#');

    // Expand shorthand hex (#RGB -> #RRGGBB)
    let hex = if hex.len() == 3 {
        format!(
            "{}{}{}{}{}{}",
            &hex[0..1], &hex[0..1],
            &hex[1..2], &hex[1..2],
            &hex[2..3], &hex[2..3]
        )
    } else {
        hex.to_string()
    };

    if hex.len() != 6 {
        return None;
    }

    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;

    Some(Color::Rgb(r, g, b))
}

/// Map semantic color names to terminal ANSI colors
/// This provides better compatibility across terminals
fn color_name_to_ansi(color_name: &str) -> Option<Color> {
    match color_name.to_lowercase().as_str() {
        "red" => Some(Color::Red),
        "orange" => Some(Color::LightRed),  // Orange -> light red
        "yellow" => Some(Color::Yellow),
        "green" => Some(Color::Green),
        "blue" => Some(Color::Blue),
        "purple" => Some(Color::Magenta),
        "pink" => Some(Color::LightMagenta),
        "gray" | "grey" => Some(Color::DarkGray),
        _ => None,
    }
}

/// Get the ratatui Color for a note's color field
/// Returns None if the note has no color or the color is invalid
pub fn get_note_color(
    note_color: Option<&String>,
    settings: &UserSettings,
    is_dark_theme: bool,
) -> Option<Color> {
    let color_key = note_color?;

    // Try to get true color from palette first (for terminals that support it)
    let palette = settings.get_color_palette();

    if let Some(color_def) = palette.get(color_key) {
        let hex = if is_dark_theme {
            &color_def.dark
        } else {
            &color_def.light
        };

        if let Some(rgb_color) = hex_to_color(hex) {
            return Some(rgb_color);
        }
    }

    // Fall back to ANSI colors for better terminal compatibility
    color_name_to_ansi(color_key)
}

/// Get the ratatui Color for a tag's color
/// Returns None if the tag has no color or the color is invalid
pub fn get_tag_color(
    tag_name: &str,
    settings: &UserSettings,
    is_dark_theme: bool,
) -> Option<Color> {
    let color_key = settings.get_tag_color(tag_name)?;

    // Try to get true color from palette first (for terminals that support it)
    let palette = settings.get_color_palette();

    if let Some(color_def) = palette.get(&color_key) {
        let hex = if is_dark_theme {
            &color_def.dark
        } else {
            &color_def.light
        };

        if let Some(rgb_color) = hex_to_color(hex) {
            return Some(rgb_color);
        }
    }

    // Fall back to ANSI colors
    color_name_to_ansi(&color_key)
}

/// Check if the current theme is dark based on the theme name
pub fn is_dark_theme(theme_name: &str) -> bool {
    !theme_name.contains("light")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_to_color_full() {
        let color = hex_to_color("#FF0000");
        assert_eq!(color, Some(Color::Rgb(255, 0, 0)));

        let color = hex_to_color("#00FF00");
        assert_eq!(color, Some(Color::Rgb(0, 255, 0)));

        let color = hex_to_color("#0000FF");
        assert_eq!(color, Some(Color::Rgb(0, 0, 255)));
    }

    #[test]
    fn test_hex_to_color_shorthand() {
        let color = hex_to_color("#F00");
        assert_eq!(color, Some(Color::Rgb(255, 0, 0)));

        let color = hex_to_color("#0F0");
        assert_eq!(color, Some(Color::Rgb(0, 255, 0)));

        let color = hex_to_color("#00F");
        assert_eq!(color, Some(Color::Rgb(0, 0, 255)));
    }

    #[test]
    fn test_hex_to_color_no_hash() {
        let color = hex_to_color("FF0000");
        assert_eq!(color, Some(Color::Rgb(255, 0, 0)));
    }

    #[test]
    fn test_hex_to_color_invalid() {
        assert_eq!(hex_to_color("#GGGGGG"), None);
        assert_eq!(hex_to_color("#FF"), None);
        assert_eq!(hex_to_color("invalid"), None);
    }

    #[test]
    fn test_is_dark_theme() {
        assert!(is_dark_theme("default-dark"));
        assert!(is_dark_theme("solarized-dark"));
        assert!(is_dark_theme("gruvbox-dark"));
        assert!(!is_dark_theme("default-light"));
        assert!(!is_dark_theme("solarized-light"));
        assert!(!is_dark_theme("gruvbox-light"));
    }

    #[test]
    fn test_get_note_color_with_default_palette() {
        let settings = crate::models::UserSettings::default();

        // Test getting a color that exists in default palette
        let color = get_note_color(Some(&"red".to_string()), &settings, true);
        assert!(color.is_some());
        assert_eq!(color, Some(Color::Red));

        // Test with light theme (ANSI colors are the same regardless of theme)
        let color_light = get_note_color(Some(&"red".to_string()), &settings, false);
        assert!(color_light.is_some());
        assert_eq!(color_light, Some(Color::Red));
    }

    #[test]
    fn test_get_note_color_none() {
        let settings = crate::models::UserSettings::default();

        // Test with no color
        let color = get_note_color(None, &settings, true);
        assert!(color.is_none());
    }

    #[test]
    fn test_get_note_color_invalid() {
        let settings = crate::models::UserSettings::default();

        // Test with invalid color name
        let color = get_note_color(Some(&"nonexistent".to_string()), &settings, true);
        assert!(color.is_none());
    }

    #[test]
    fn test_get_tag_color() {
        use std::collections::HashMap;

        let mut settings = crate::models::UserSettings::default();

        // Set up tag colors
        let mut tag_colors = HashMap::new();
        tag_colors.insert("work".to_string(), "blue".to_string());
        tag_colors.insert("personal".to_string(), "green".to_string());
        settings.tag_colors = Some(tag_colors);

        // Test getting a tag color
        let color = get_tag_color("work", &settings, true);
        assert!(color.is_some());

        // Test tag without color
        let no_color = get_tag_color("uncolored", &settings, true);
        assert!(no_color.is_none());

        // Test case insensitivity
        let color_uppercase = get_tag_color("WORK", &settings, true);
        assert!(color_uppercase.is_some());
    }

    #[test]
    fn test_get_tag_color_no_mappings() {
        let settings = crate::models::UserSettings::default();

        // Test with no tag color mappings
        let color = get_tag_color("anytag", &settings, true);
        assert!(color.is_none());
    }

    #[test]
    fn test_all_default_colors_valid() {
        let settings = crate::models::UserSettings::default();
        let palette = settings.get_color_palette();

        // Test that all default color names map to ANSI colors
        for (color_name, _color_def) in palette.iter() {
            let ansi_color = color_name_to_ansi(color_name);
            assert!(
                ansi_color.is_some(),
                "Color name '{}' should map to ANSI color", color_name
            );
        }
    }

    #[test]
    fn test_color_name_to_ansi_mapping() {
        // Test all 8 basic colors
        assert_eq!(color_name_to_ansi("red"), Some(Color::Red));
        assert_eq!(color_name_to_ansi("orange"), Some(Color::LightRed));
        assert_eq!(color_name_to_ansi("yellow"), Some(Color::Yellow));
        assert_eq!(color_name_to_ansi("green"), Some(Color::Green));
        assert_eq!(color_name_to_ansi("blue"), Some(Color::Blue));
        assert_eq!(color_name_to_ansi("purple"), Some(Color::Magenta));
        assert_eq!(color_name_to_ansi("pink"), Some(Color::LightMagenta));
        assert_eq!(color_name_to_ansi("gray"), Some(Color::DarkGray));
        assert_eq!(color_name_to_ansi("grey"), Some(Color::DarkGray));

        // Test case insensitivity
        assert_eq!(color_name_to_ansi("RED"), Some(Color::Red));
        assert_eq!(color_name_to_ansi("Blue"), Some(Color::Blue));

        // Test invalid color
        assert_eq!(color_name_to_ansi("invalid"), None);
    }
}
