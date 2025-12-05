use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};

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
                "SELECT language, theme, sort_order, auto_lock_timeout, sync_enabled, sync_endpoint
                 FROM settings WHERE id = 1",
                [],
                |row| {
                    Ok(UserSettings {
                        language: row.get(0)?,
                        theme: parse_theme(&row.get::<_, String>(1)?),
                        sort_order: parse_sort_order(&row.get::<_, String>(2)?),
                        auto_lock_timeout: row.get(3)?,
                        sync_enabled: row.get::<_, i32>(4)? != 0,
                        sync_endpoint: row.get(5)?,
                    })
                },
            )
            .optional()?;

        Ok(result.unwrap_or_else(UserSettings::default))
    }

    /// Update user settings
    pub fn update(&self, settings: &UserSettings) -> Result<()> {
        settings.validate().map_err(|e| anyhow::anyhow!("{}", e))?;

        self.conn.execute(
            "INSERT OR REPLACE INTO settings (id, language, theme, sort_order, auto_lock_timeout, sync_enabled, sync_endpoint)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                &settings.language,
                settings.theme.to_string(),
                settings.sort_order.to_string(),
                settings.auto_lock_timeout,
                settings.sync_enabled as i32,
                &settings.sync_endpoint,
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
        if minutes < 1 || minutes > 1440 {
            anyhow::bail!("Auto-lock timeout must be between 1 and 1440 minutes");
        }

        self.conn.execute(
            "UPDATE settings SET auto_lock_timeout = ?1 WHERE id = 1",
            params![minutes],
        )?;
        Ok(())
    }
}

/// Parse theme string
fn parse_theme(s: &str) -> Theme {
    match s.to_lowercase().as_str() {
        "light" => Theme::Light,
        "dark" => Theme::Dark,
        _ => Theme::Auto,
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
