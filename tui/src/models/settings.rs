use serde::{Deserialize, Serialize};

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
        }
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
