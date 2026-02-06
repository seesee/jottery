//! Input validation utilities
//!
//! Provides validation functions for user input to prevent DoS attacks
//! and ensure data integrity.

use crate::{config::Config, error::AppError};

/// Validate device name length
pub fn validate_device_name(name: &str, config: &Config) -> Result<(), AppError> {
    if name.trim().is_empty() {
        return Err(AppError::BadRequest("Device name cannot be empty".to_string()));
    }
    if name.len() > config.max_device_name_length {
        return Err(AppError::BadRequest(format!(
            "Device name exceeds maximum length of {} characters",
            config.max_device_name_length
        )));
    }
    Ok(())
}

/// Validate inbox content size
pub fn validate_inbox_content(content: &str, config: &Config) -> Result<(), AppError> {
    if content.trim().is_empty() {
        return Err(AppError::BadRequest("Content must not be empty".to_string()));
    }
    if content.len() > config.max_inbox_content_size {
        return Err(AppError::PayloadTooLarge(format!(
            "Content exceeds maximum size of {} bytes",
            config.max_inbox_content_size
        )));
    }
    Ok(())
}

/// Validate note content size
pub fn validate_note_content(content: &str, config: &Config) -> Result<(), AppError> {
    if content.len() > config.max_note_content_size {
        return Err(AppError::PayloadTooLarge(format!(
            "Note content exceeds maximum size of {} bytes",
            config.max_note_content_size
        )));
    }
    Ok(())
}

/// Validate tags (individual length and count)
pub fn validate_tags(tags: &[String], config: &Config) -> Result<(), AppError> {
    if tags.len() > config.max_tags_per_note {
        return Err(AppError::BadRequest(format!(
            "Too many tags (maximum {})",
            config.max_tags_per_note
        )));
    }
    for tag in tags {
        if tag.len() > config.max_tag_length {
            return Err(AppError::BadRequest(format!(
                "Tag '{}...' exceeds maximum length of {} characters",
                &tag[..20.min(tag.len())],
                config.max_tag_length
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        Config {
            database_url: String::new(),
            port: 3030,
            max_payload_size: 100_000_000,
            cors_allowed_origins: None,
            session_expiry_days: 7,
            default_admin_email: String::new(),
            default_admin_password: String::new(),
            argon2_m_cost: 19456,
            argon2_t_cost: 2,
            argon2_p_cost: 1,
            default_storage_quota_mb: 1000,
            default_max_upload_size_mb: 5,
            default_inbox_max_items: 100,
            default_inbox_max_size_mb: 10,
            password_complexity: "standard".to_string(),
            enable_hsts: false,
            max_device_name_length: 255,
            max_inbox_content_size: 1_048_576,
            max_note_content_size: 10_485_760,
            max_tag_length: 100,
            max_tags_per_note: 50,
        }
    }

    #[test]
    fn test_validate_device_name() {
        let config = test_config();

        // Valid names
        assert!(validate_device_name("My Device", &config).is_ok());
        assert!(validate_device_name("a", &config).is_ok());

        // Empty name
        assert!(validate_device_name("", &config).is_err());
        assert!(validate_device_name("   ", &config).is_err());

        // Too long
        let long_name = "a".repeat(256);
        assert!(validate_device_name(&long_name, &config).is_err());
    }

    #[test]
    fn test_validate_inbox_content() {
        let config = test_config();

        // Valid content
        assert!(validate_inbox_content("Hello", &config).is_ok());

        // Empty content
        assert!(validate_inbox_content("", &config).is_err());
        assert!(validate_inbox_content("   ", &config).is_err());

        // Too large
        let large_content = "a".repeat(1_048_577);
        assert!(validate_inbox_content(&large_content, &config).is_err());
    }

    #[test]
    fn test_validate_tags() {
        let config = test_config();

        // Valid tags
        assert!(validate_tags(&["tag1".to_string(), "tag2".to_string()], &config).is_ok());

        // Too many tags
        let many_tags: Vec<String> = (0..51).map(|i| format!("tag{}", i)).collect();
        assert!(validate_tags(&many_tags, &config).is_err());

        // Tag too long
        let long_tag = "a".repeat(101);
        assert!(validate_tags(&[long_tag], &config).is_err());
    }
}
