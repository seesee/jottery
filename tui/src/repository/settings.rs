#![allow(dead_code)]
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use base64::{Engine as _, engine::general_purpose};

use crate::models::{SortOrder, Theme, UserSettings, ColorPalette};
use crate::crypto::CryptoService;

/// Encrypt displayNames in color palette
fn encrypt_color_palette(palette: &ColorPalette, key: &[u8; 32]) -> Result<ColorPalette> {
    let crypto = CryptoService::new();
    let mut encrypted = palette.clone();

    for (_color_key, color_def) in encrypted.iter_mut() {
        if let Some(display_name) = &color_def.display_name {
            let encrypted_name = crypto.encrypt_text(display_name, key)?;
            // Store as JSON string of EncryptedData
            color_def.display_name = Some(serde_json::to_string(&encrypted_name)?);
        }
    }

    Ok(encrypted)
}

/// Decrypt displayNames in color palette
fn decrypt_color_palette(palette: &ColorPalette, key: &[u8; 32]) -> Result<ColorPalette> {
    let crypto = CryptoService::new();
    let mut decrypted = palette.clone();

    for (_color_key, color_def) in decrypted.iter_mut() {
        if let Some(encrypted_display_name) = &color_def.display_name {
            // Try to parse as encrypted data
            if let Ok(encrypted_data) = serde_json::from_str(encrypted_display_name) {
                // Decrypt it
                if let Ok(decrypted_name) = crypto.decrypt_text(&encrypted_data, key) {
                    color_def.display_name = Some(decrypted_name);
                }
                // If decryption fails, leave it as is (fallback)
            }
            // If not JSON, it's legacy unencrypted data, leave as is
        }
    }

    Ok(decrypted)
}

/// Repository for settings operations
pub struct SettingsRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SettingsRepository<'a> {
    /// Create a new settings repository
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Get user settings (or default if not exists)
    /// If key is provided, decrypts colorPalette displayNames
    pub fn get_with_key(&self, key: Option<&[u8; 32]>) -> Result<UserSettings> {
        let result = self.conn
            .query_row(
                "SELECT language, theme, sort_order, auto_lock_timeout, sync_enabled, sync_endpoint, auto_sync_interval_minutes, remember_password, stored_password, color_palette, tag_colors
                 FROM settings WHERE id = 1",
                [],
                |row| {
                    // Decode stored password if present
                    let stored_password: Option<String> = row.get(8)?;
                    let decoded_password = stored_password.map(|p| decode_password(&p));

                    // Parse color_palette JSON if present
                    let color_palette_json: Option<String> = row.get(9)?;
                    let color_palette = color_palette_json
                        .and_then(|json| serde_json::from_str(&json).ok());

                    // Parse tag_colors JSON if present
                    let tag_colors_json: Option<String> = row.get(10)?;
                    let tag_colors = tag_colors_json
                        .and_then(|json| serde_json::from_str(&json).ok());

                    let mut settings = UserSettings {
                        language: row.get(0)?,
                        theme: parse_theme(&row.get::<_, String>(1)?),
                        sort_order: parse_sort_order(&row.get::<_, String>(2)?),
                        auto_lock_timeout: row.get(3)?,
                        sync_enabled: row.get::<_, i32>(4)? != 0,
                        sync_endpoint: row.get(5)?,
                        auto_sync_interval_minutes: row.get::<_, Option<i32>>(6)?.unwrap_or(1),
                        remember_password: row.get::<_, Option<i32>>(7)?.unwrap_or(0) != 0,
                        stored_password: decoded_password,
                        color_palette,
                        tag_colors,
                    };

                    // Decrypt color palette displayNames if key is provided
                    if let (Some(key), Some(palette)) = (key, &settings.color_palette) {
                        if let Ok(decrypted) = decrypt_color_palette(palette, key) {
                            settings.color_palette = Some(decrypted);
                        }
                    }

                    Ok(settings)
                },
            )
            .optional()?;

        Ok(result.unwrap_or_else(UserSettings::default))
    }

    /// Get user settings without decryption (for backward compatibility)
    pub fn get(&self) -> Result<UserSettings> {
        self.get_with_key(None)
    }

    /// Update user settings
    /// If key is provided, encrypts colorPalette displayNames before saving
    pub fn update_with_key(&self, settings: &UserSettings, key: Option<&[u8; 32]>) -> Result<()> {
        settings.validate().map_err(|e| anyhow::anyhow!("{}", e))?;

        // Encode password if present
        let encoded_password = settings.stored_password.as_ref().map(|p| encode_password(p));

        // Encrypt and serialize color_palette to JSON if present
        let color_palette_json = if let Some(palette) = &settings.color_palette {
            // Encrypt displayNames if key is provided
            let palette_to_save = if let Some(k) = key {
                encrypt_color_palette(palette, k)?
            } else {
                palette.clone()
            };
            Some(serde_json::to_string(&palette_to_save)?)
        } else {
            None
        };

        // Serialize tag_colors to JSON if present
        let tag_colors_json = settings.tag_colors.as_ref()
            .map(|tc| serde_json::to_string(tc))
            .transpose()?;

        self.conn.execute(
            "INSERT OR REPLACE INTO settings (id, language, theme, sort_order, auto_lock_timeout, sync_enabled, sync_endpoint, auto_sync_interval_minutes, remember_password, stored_password, color_palette, tag_colors)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                &settings.language,
                settings.theme.to_string(),
                settings.sort_order.to_string(),
                settings.auto_lock_timeout,
                settings.sync_enabled as i32,
                &settings.sync_endpoint,
                settings.auto_sync_interval_minutes,
                settings.remember_password as i32,
                &encoded_password,
                &color_palette_json,
                &tag_colors_json,
            ],
        )?;

        Ok(())
    }

    /// Update user settings without encryption (for backward compatibility)
    pub fn update(&self, settings: &UserSettings) -> Result<()> {
        self.update_with_key(settings, None)
    }

    /// Update only specific fields
    pub fn update_field(&self, field: &str, value: &str) -> Result<()> {
        let query = format!("UPDATE settings SET {} = ?1 WHERE id = 1", field);
        self.conn.execute(&query, params![value])?;
        Ok(())
    }

    /// Enable/disable sync
    pub fn set_sync_enabled(&self, enabled: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE settings SET sync_enabled = ?1 WHERE id = 1",
            params![enabled as i32],
        )?;
        Ok(())
    }

    /// Set sync endpoint
    pub fn set_sync_endpoint(&self, endpoint: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE settings SET sync_endpoint = ?1 WHERE id = 1",
            params![endpoint],
        )?;
        Ok(())
    }

    /// Set theme
    pub fn set_theme(&self, theme: Theme) -> Result<()> {
        self.conn.execute(
            "UPDATE settings SET theme = ?1 WHERE id = 1",
            params![theme.to_string()],
        )?;
        Ok(())
    }

    /// Set sort order
    pub fn set_sort_order(&self, sort_order: SortOrder) -> Result<()> {
        self.conn.execute(
            "UPDATE settings SET sort_order = ?1 WHERE id = 1",
            params![sort_order.to_string()],
        )?;
        Ok(())
    }

    /// Set auto-lock timeout (in minutes)
    pub fn set_auto_lock_timeout(&self, minutes: i32) -> Result<()> {
        if !(1..=1440).contains(&minutes) {
            anyhow::bail!("Auto-lock timeout must be between 1 and 1440 minutes");
        }

        self.conn.execute(
            "UPDATE settings SET auto_lock_timeout = ?1 WHERE id = 1",
            params![minutes],
        )?;
        Ok(())
    }
}

/// Parse theme string (with backward compatibility for old values)
fn parse_theme(s: &str) -> Theme {
    match s.to_lowercase().as_str() {
        // Legacy compatibility - map old values to new scheme names
        "light" => Theme::from_name("default-light"),
        "dark" => Theme::from_name("default-dark"),
        "auto" => Theme::from_name("default-dark"),
        // Handle display names from web client
        "default dark" => Theme::from_name("default-dark"),
        "default light" => Theme::from_name("default-light"),
        // New color scheme names
        _ => Theme::from_name(s),
    }
}

/// Parse sort order string
fn parse_sort_order(s: &str) -> SortOrder {
    match s.to_lowercase().as_str() {
        "oldest" => SortOrder::Oldest,
        "alpha" => SortOrder::Alpha,
        "created" => SortOrder::Created,
        _ => SortOrder::Recent,
    }
}

/// Encode password with base64 for minimal obfuscation (NOT encryption)
/// Note: This provides basic obfuscation only, not real security
fn encode_password(password: &str) -> String {
    general_purpose::STANDARD.encode(password.as_bytes())
}

/// Decode base64 encoded password
/// Handles both encoded and plaintext for backward compatibility
fn decode_password(stored: &str) -> String {
    // Try to decode as base64
    match general_purpose::STANDARD.decode(stored.as_bytes()) {
        Ok(decoded) => {
            // Successfully decoded, convert to string
            match String::from_utf8(decoded) {
                Ok(password) => password,
                Err(_) => {
                    // Not valid UTF-8 after decoding, assume it's plaintext
                    stored.to_string()
                }
            }
        }
        Err(_) => {
            // Not valid base64, assume it's plaintext (backward compatibility)
            stored.to_string()
        }
    }
}
