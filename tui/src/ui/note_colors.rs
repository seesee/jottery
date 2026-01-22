//! Note color utilities for rendering colored notes and tags in the TUI
//!
//! This module provides functions to convert hex colors from the user's color palette
//! to ratatui Color types for terminal rendering.

use ratatui::style::Color;
use crate::models::UserSettings;

/// Convert a hex color string to a ratatui Color
/// Supports formats: #RRGGBB or #RGB
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

/// Get the ratatui Color for a note's color field
/// Returns None if the note has no color or the color is invalid
pub fn get_note_color(
    note_color: Option<&String>,
    settings: &UserSettings,
    is_dark_theme: bool,
) -> Option<Color> {
    let color_key = note_color?;
    let palette = settings.get_color_palette();
    let color_def = palette.get(color_key)?;

    let hex = if is_dark_theme {
        &color_def.dark
    } else {
        &color_def.light
    };

    hex_to_color(hex)
}

/// Get the ratatui Color for a tag's color
/// Returns None if the tag has no color or the color is invalid
pub fn get_tag_color(
    tag_name: &str,
    settings: &UserSettings,
    is_dark_theme: bool,
) -> Option<Color> {
    let color_key = settings.get_tag_color(tag_name)?;
    let palette = settings.get_color_palette();
    let color_def = palette.get(&color_key)?;

    let hex = if is_dark_theme {
        &color_def.dark
    } else {
        &color_def.light
    };

    hex_to_color(hex)
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
}
