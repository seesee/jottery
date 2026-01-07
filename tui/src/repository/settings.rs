#![allow(dead_code)]
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use base64::{Engine as _, engine::general_purpose};

use crate::models::{SortOrder, Theme, UserSettings};

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
    pub fn get(&self) -> Result<UserSettings> {
        let result = self.conn
            .query_row(
                "SELECT language, theme, sort_order, auto_lock_timeout, sync_enabled, sync_endpoint, auto_sync_interval_minutes, remember_password, stored_password
                 FROM settings WHERE id = 1",
                [],
                |row| {
                    // Decode stored password if present
                    let stored_password: Option<String> = row.get(8)?;
                    let decoded_password = stored_password.map(|p| decode_password(&p));

                    Ok(UserSettings {
                        language: row.get(0)?,
                        theme: parse_theme(&row.get::<_, String>(1)?),
                        sort_order: parse_sort_order(&row.get::<_, String>(2)?),
                        auto_lock_timeout: row.get(3)?,
                        sync_enabled: row.get::<_, i32>(4)? != 0,
                        sync_endpoint: row.get(5)?,
                        auto_sync_interval_minutes: row.get::<_, Option<i32>>(6)?.unwrap_or(1),
                        remember_password: row.get::<_, Option<i32>>(7)?.unwrap_or(0) != 0,
                        stored_password: decoded_password,
                    })
                },
            )
            .optional()?;

        Ok(result.unwrap_or_else(UserSettings::default))
    }

    /// Update user settings
    pub fn update(&self, settings: &UserSettings) -> Result<()> {
        settings.validate().map_err(|e| anyhow::anyhow!("{}", e))?;

        // Encode password if present
        let encoded_password = settings.stored_password.as_ref().map(|p| encode_password(p));

        self.conn.execute(
            "INSERT OR REPLACE INTO settings (id, language, theme, sort_order, auto_lock_timeout, sync_enabled, sync_endpoint, auto_sync_interval_minutes, remember_password, stored_password)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
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
            ],
        )?;

        Ok(())
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
