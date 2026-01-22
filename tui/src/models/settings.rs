use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Color definition for light and dark modes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorDefinition {
    pub light: String,       // Hex color for light mode
    pub dark: String,        // Hex color for dark mode
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "displayName")]
    pub display_name: Option<String>, // Optional user-defined display name
}

/// Color palette mapping color names to light/dark hex values
pub type ColorPalette = HashMap<String, ColorDefinition>;

/// User application settings
/// Stored unencrypted in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettings {
    pub language: String,
    pub theme: Theme,
    pub sort_order: SortOrder,
    pub auto_lock_timeout: i32, // Minutes
    pub sync_enabled: bool,
    pub sync_endpoint: Option<String>,
    #[serde(default = "default_auto_sync_interval")]
    pub auto_sync_interval_minutes: i32, // Minutes between auto-syncs (0 = disabled)
    #[serde(default)]
    pub remember_password: bool, // Whether to store password for auto-unlock
    pub stored_password: Option<String>, // Encrypted password (JSON encrypted data)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "colorPalette")]
    pub color_palette: Option<ColorPalette>, // User-customized color palette
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "tagColors")]
    pub tag_colors: Option<HashMap<String, String>>, // Tag name → color name mapping
}

fn default_auto_sync_interval() -> i32 {
    1 // Default to 1 minute for responsive sync
}

/// Theme options (color scheme names)
/// Changed from Light/Dark/Auto to full color scheme names
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Theme {
    /// New color scheme format
    Scheme(String),
}

impl Default for Theme {
    fn default() -> Self {
        Self::Scheme("default-dark".to_string())
    }
}

impl std::fmt::Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Scheme(name) => {
                // Convert internal name to display name
                use crate::ui::ColorScheme;
                write!(f, "{}", ColorScheme::internal_to_display(name))
            }
        }
    }
}

impl Theme {
    /// Get the internal scheme name
    pub fn scheme_name(&self) -> &str {
        match self {
            Self::Scheme(name) => name,
        }
    }

    /// Create from internal name
    pub fn from_name(name: &str) -> Self {
        Self::Scheme(name.to_string())
    }

    /// Cycle to next color scheme
    pub fn cycle_next(&mut self) {
        let schemes = vec![
            "default-dark",
            "default-light",
            "monokai",
            "solarized-dark",
            "solarized-light",
            "nord",
            "dracula",
            "gruvbox-dark",
            "gruvbox-light",
            "tokyo-night",
            "catppuccin",
        ];

        let current = self.scheme_name();
        let current_idx = schemes.iter().position(|&s| s == current).unwrap_or(0);
        let next_idx = (current_idx + 1) % schemes.len();
        *self = Self::Scheme(schemes[next_idx].to_string());
    }

    /// Cycle to previous color scheme
    pub fn cycle_prev(&mut self) {
        let schemes = vec![
            "default-dark",
            "default-light",
            "monokai",
            "solarized-dark",
            "solarized-light",
            "nord",
            "dracula",
            "gruvbox-dark",
            "gruvbox-light",
            "tokyo-night",
            "catppuccin",
        ];

        let current = self.scheme_name();
        let current_idx = schemes.iter().position(|&s| s == current).unwrap_or(0);
        let prev_idx = if current_idx == 0 { schemes.len() - 1 } else { current_idx - 1 };
        *self = Self::Scheme(schemes[prev_idx].to_string());
    }
}

/// Sort options for note list
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum SortOrder {
    #[default]
    Recent,    // Most recently modified first
    Oldest,    // Oldest modified first
    Alpha,     // Alphabetical by content preview
    Created,   // Most recently created first
}


impl std::fmt::Display for SortOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Recent => write!(f, "recent"),
            Self::Oldest => write!(f, "oldest"),
            Self::Alpha => write!(f, "alpha"),
            Self::Created => write!(f, "created"),
        }
    }
}

impl SortOrder {
    /// Get the next sort order in the cycle
    pub fn next(self) -> Self {
        match self {
            Self::Recent => Self::Oldest,
            Self::Oldest => Self::Alpha,
            Self::Alpha => Self::Created,
            Self::Created => Self::Recent,
        }
    }

    /// Get the previous sort order in the cycle
    pub fn prev(self) -> Self {
        match self {
            Self::Recent => Self::Created,
            Self::Oldest => Self::Recent,
            Self::Alpha => Self::Oldest,
            Self::Created => Self::Alpha,
        }
    }
}

impl UserSettings {
    /// Create default settings
    pub fn default() -> Self {
        Self {
            language: "en-GB".to_string(),
            theme: Theme::default(),
            sort_order: SortOrder::Recent,
            auto_lock_timeout: 15, // 15 minutes
            sync_enabled: false,
            sync_endpoint: None,
            auto_sync_interval_minutes: 1, // 1 minute for responsive sync
            remember_password: false, // Disabled by default for security
            stored_password: None,
            color_palette: None, // Will use default palette if None
            tag_colors: None,    // No tag colors by default
        }
    }

    /// Get default color palette
    pub fn default_color_palette() -> ColorPalette {
        let mut palette = HashMap::new();

        palette.insert("red".to_string(), ColorDefinition {
            light: "#FFE5E5".to_string(),
            dark: "#5C1A1A".to_string(),
            display_name: None,
        });
        palette.insert("orange".to_string(), ColorDefinition {
            light: "#FFF0E0".to_string(),
            dark: "#5C3A1A".to_string(),
            display_name: None,
        });
        palette.insert("yellow".to_string(), ColorDefinition {
            light: "#FFFACD".to_string(),
            dark: "#5C5520".to_string(),
            display_name: None,
        });
        palette.insert("green".to_string(), ColorDefinition {
            light: "#E5F5E5".to_string(),
            dark: "#1A4D1A".to_string(),
            display_name: None,
        });
        palette.insert("blue".to_string(), ColorDefinition {
            light: "#E5F0FF".to_string(),
            dark: "#1A3A5C".to_string(),
            display_name: None,
        });
        palette.insert("purple".to_string(), ColorDefinition {
            light: "#F0E5FF".to_string(),
            dark: "#3A1A5C".to_string(),
            display_name: None,
        });
        palette.insert("pink".to_string(), ColorDefinition {
            light: "#FFE5F0".to_string(),
            dark: "#5C1A3A".to_string(),
            display_name: None,
        });
        palette.insert("gray".to_string(), ColorDefinition {
            light: "#F0F0F0".to_string(),
            dark: "#2A2A2A".to_string(),
            display_name: None,
        });

        palette
    }

    /// Get color palette (returns default if not set)
    pub fn get_color_palette(&self) -> ColorPalette {
        self.color_palette.clone().unwrap_or_else(Self::default_color_palette)
    }

    /// Get tag color for a given tag name (case-insensitive)
    pub fn get_tag_color(&self, tag_name: &str) -> Option<String> {
        self.tag_colors.as_ref()?.get(&tag_name.to_lowercase()).cloned()
    }

    /// Get color display name or fallback to key
    pub fn get_color_display_name(&self, color_key: &str) -> String {
        let palette = self.get_color_palette();
        palette.get(color_key)
            .and_then(|def| def.display_name.clone())
            .unwrap_or_else(|| color_key.to_string())
    }

    /// Find color key by display name (case-insensitive)
    pub fn get_color_key_by_display_name(&self, display_name: &str) -> Option<String> {
        let palette = self.get_color_palette();
        let search = display_name.to_lowercase();

        for (key, def) in palette.iter() {
            let name = def.display_name.as_ref().unwrap_or(key).to_lowercase();
            if name == search {
                return Some(key.clone());
            }
        }

        None
    }

    /// Validate settings
    pub fn validate(&self) -> Result<(), String> {
        if self.auto_lock_timeout < 1 || self.auto_lock_timeout > 1440 {
            return Err("Auto-lock timeout must be between 1 and 1440 minutes".to_string());
        }

        if self.auto_sync_interval_minutes < 0 || self.auto_sync_interval_minutes > 1440 {
            return Err("Auto-sync interval must be between 0 and 1440 minutes".to_string());
        }

        if self.sync_enabled && self.sync_endpoint.is_none() {
            return Err("Sync endpoint is required when sync is enabled".to_string());
        }

        if let Some(endpoint) = &self.sync_endpoint {
            if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
                return Err("Sync endpoint must start with http:// or https://".to_string());
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = UserSettings::default();
        assert_eq!(settings.language, "en-GB");
        assert_eq!(settings.theme.scheme_name(), "default-dark");
        assert_eq!(settings.sort_order, SortOrder::Recent);
        assert_eq!(settings.auto_lock_timeout, 15);
        assert!(!settings.sync_enabled);
        assert_eq!(settings.auto_sync_interval_minutes, 1); // Default to 1 minute
    }

    #[test]
    fn test_settings_validation() {
        let mut settings = UserSettings::default();
        assert!(settings.validate().is_ok());

        // Invalid timeout
        settings.auto_lock_timeout = 0;
        assert!(settings.validate().is_err());

        settings.auto_lock_timeout = 15;

        // Sync enabled without endpoint
        settings.sync_enabled = true;
        assert!(settings.validate().is_err());

        // Valid with endpoint
        settings.sync_endpoint = Some("https://example.com".to_string());
        assert!(settings.validate().is_ok());

        // Invalid endpoint protocol
        settings.sync_endpoint = Some("ftp://example.com".to_string());
        assert!(settings.validate().is_err());
    }

    #[test]
    fn test_auto_sync_interval_validation() {
        let mut settings = UserSettings::default();
        settings.sync_endpoint = Some("https://example.com".to_string());

        // Valid: 0 (disabled)
        settings.auto_sync_interval_minutes = 0;
        assert!(settings.validate().is_ok());

        // Valid: 1 minute (default)
        settings.auto_sync_interval_minutes = 1;
        assert!(settings.validate().is_ok());

        // Valid: 1440 minutes (24 hours)
        settings.auto_sync_interval_minutes = 1440;
        assert!(settings.validate().is_ok());

        // Invalid: negative
        settings.auto_sync_interval_minutes = -1;
        assert!(settings.validate().is_err());

        // Invalid: over 24 hours
        settings.auto_sync_interval_minutes = 1441;
        assert!(settings.validate().is_err());
    }
}
