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
}

/// Theme options
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    Auto,
}

impl Default for Theme {
    fn default() -> Self {
        Self::Auto
    }
}

impl std::fmt::Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Light => write!(f, "light"),
            Self::Dark => write!(f, "dark"),
            Self::Auto => write!(f, "auto"),
        }
    }
}

/// Sort options for note list
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Recent,    // Most recently modified first
    Oldest,    // Oldest modified first
    Alpha,     // Alphabetical by content preview
    Created,   // Most recently created first
}

impl Default for SortOrder {
    fn default() -> Self {
        Self::Recent
    }
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

impl UserSettings {
    /// Create default settings
    pub fn default() -> Self {
        Self {
            language: "en-GB".to_string(),
            theme: Theme::Auto,
            sort_order: SortOrder::Recent,
            auto_lock_timeout: 15, // 15 minutes
            sync_enabled: false,
            sync_endpoint: None,
        }
    }

    /// Validate settings
    pub fn validate(&self) -> Result<(), String> {
        if self.auto_lock_timeout < 1 || self.auto_lock_timeout > 1440 {
            return Err("Auto-lock timeout must be between 1 and 1440 minutes".to_string());
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
        assert_eq!(settings.theme, Theme::Auto);
        assert_eq!(settings.sort_order, SortOrder::Recent);
        assert_eq!(settings.auto_lock_timeout, 15);
        assert!(!settings.sync_enabled);
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
}
