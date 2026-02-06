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

    // Small config for easier boundary testing
    fn small_config() -> Config {
        Config {
            max_device_name_length: 10,
            max_inbox_content_size: 100,
            max_note_content_size: 200,
            max_tag_length: 5,
            max_tags_per_note: 3,
            ..test_config()
        }
    }

    // =========================================================================
    // Device name validation
    // =========================================================================

    #[test]
    fn test_validate_device_name_basic() {
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
    fn test_validate_device_name_boundary() {
        let config = small_config(); // max_device_name_length = 10

        // Exactly at limit (10 chars)
        assert!(validate_device_name("1234567890", &config).is_ok());

        // One below limit (9 chars)
        assert!(validate_device_name("123456789", &config).is_ok());

        // One above limit (11 chars)
        assert!(validate_device_name("12345678901", &config).is_err());
    }

    #[test]
    fn test_validate_device_name_unicode() {
        let config = small_config(); // max_device_name_length = 10

        // Unicode: 日本語 is 3 characters but 9 bytes in UTF-8
        // Validation uses .len() which counts bytes, not characters
        assert!(validate_device_name("日本語", &config).is_ok()); // 9 bytes

        // 4 Japanese chars = 12 bytes, exceeds 10-byte limit
        assert!(validate_device_name("日本語語", &config).is_err()); // 12 bytes

        // Emoji: 🎉 is 4 bytes
        assert!(validate_device_name("🎉🎉", &config).is_ok()); // 8 bytes
        assert!(validate_device_name("🎉🎉🎉", &config).is_err()); // 12 bytes
    }

    #[test]
    fn test_validate_device_name_whitespace_only() {
        let config = test_config();

        assert!(validate_device_name(" ", &config).is_err());
        assert!(validate_device_name("\t", &config).is_err());
        assert!(validate_device_name("\n", &config).is_err());
        assert!(validate_device_name("  \t  \n  ", &config).is_err());
    }

    #[test]
    fn test_validate_device_name_with_special_chars() {
        let config = test_config();

        // Special characters should be allowed
        assert!(validate_device_name("My-Device_01", &config).is_ok());
        assert!(validate_device_name("Device (Home)", &config).is_ok());
        assert!(validate_device_name("Device's Name", &config).is_ok());
        assert!(validate_device_name("Device \"Test\"", &config).is_ok());
    }

    // =========================================================================
    // Inbox content validation
    // =========================================================================

    #[test]
    fn test_validate_inbox_content_basic() {
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
    fn test_validate_inbox_content_boundary() {
        let config = small_config(); // max_inbox_content_size = 100

        // Exactly at limit
        let at_limit = "a".repeat(100);
        assert!(validate_inbox_content(&at_limit, &config).is_ok());

        // One below limit
        let below_limit = "a".repeat(99);
        assert!(validate_inbox_content(&below_limit, &config).is_ok());

        // One above limit
        let above_limit = "a".repeat(101);
        assert!(validate_inbox_content(&above_limit, &config).is_err());
    }

    #[test]
    fn test_validate_inbox_content_unicode() {
        let config = small_config(); // max_inbox_content_size = 100

        // 33 Japanese chars = 99 bytes (at limit)
        let jp_at_limit = "あ".repeat(33); // 33 * 3 = 99 bytes
        assert!(validate_inbox_content(&jp_at_limit, &config).is_ok());

        // 34 Japanese chars = 102 bytes (over limit)
        let jp_over_limit = "あ".repeat(34); // 34 * 3 = 102 bytes
        assert!(validate_inbox_content(&jp_over_limit, &config).is_err());
    }

    #[test]
    fn test_validate_inbox_content_minimum() {
        let config = test_config();

        // Single character is valid
        assert!(validate_inbox_content("a", &config).is_ok());
        assert!(validate_inbox_content(".", &config).is_ok());
        assert!(validate_inbox_content("あ", &config).is_ok());
    }

    // =========================================================================
    // Note content validation
    // =========================================================================

    #[test]
    fn test_validate_note_content_boundary() {
        let config = small_config(); // max_note_content_size = 200

        // Exactly at limit
        let at_limit = "a".repeat(200);
        assert!(validate_note_content(&at_limit, &config).is_ok());

        // One below limit
        let below_limit = "a".repeat(199);
        assert!(validate_note_content(&below_limit, &config).is_ok());

        // One above limit
        let above_limit = "a".repeat(201);
        assert!(validate_note_content(&above_limit, &config).is_err());
    }

    #[test]
    fn test_validate_note_content_empty_allowed() {
        let config = test_config();

        // Empty note content is allowed (unlike inbox content)
        assert!(validate_note_content("", &config).is_ok());
        assert!(validate_note_content("   ", &config).is_ok());
    }

    #[test]
    fn test_validate_note_content_unicode() {
        let config = small_config(); // max_note_content_size = 200

        // 66 Japanese chars = 198 bytes (at limit)
        let jp_at_limit = "あ".repeat(66); // 66 * 3 = 198 bytes
        assert!(validate_note_content(&jp_at_limit, &config).is_ok());

        // 67 Japanese chars = 201 bytes (over limit)
        let jp_over_limit = "あ".repeat(67); // 67 * 3 = 201 bytes
        assert!(validate_note_content(&jp_over_limit, &config).is_err());
    }

    // =========================================================================
    // Tag validation
    // =========================================================================

    #[test]
    fn test_validate_tags_basic() {
        let config = test_config();

        // Valid tags
        assert!(validate_tags(&["tag1".to_string(), "tag2".to_string()], &config).is_ok());

        // Empty list is valid
        assert!(validate_tags(&[], &config).is_ok());

        // Too many tags
        let many_tags: Vec<String> = (0..51).map(|i| format!("tag{}", i)).collect();
        assert!(validate_tags(&many_tags, &config).is_err());

        // Tag too long
        let long_tag = "a".repeat(101);
        assert!(validate_tags(&[long_tag], &config).is_err());
    }

    #[test]
    fn test_validate_tags_count_boundary() {
        let config = small_config(); // max_tags_per_note = 3

        // Exactly at limit
        let at_limit: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        assert!(validate_tags(&at_limit, &config).is_ok());

        // One below limit
        let below_limit: Vec<String> = vec!["a".into(), "b".into()];
        assert!(validate_tags(&below_limit, &config).is_ok());

        // One above limit
        let above_limit: Vec<String> = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        assert!(validate_tags(&above_limit, &config).is_err());
    }

    #[test]
    fn test_validate_tags_length_boundary() {
        let config = small_config(); // max_tag_length = 5

        // Tag exactly at limit
        assert!(validate_tags(&["12345".to_string()], &config).is_ok());

        // Tag one below limit
        assert!(validate_tags(&["1234".to_string()], &config).is_ok());

        // Tag one above limit
        assert!(validate_tags(&["123456".to_string()], &config).is_err());
    }

    #[test]
    fn test_validate_tags_unicode() {
        let config = small_config(); // max_tag_length = 5

        // Japanese: あ is 3 bytes
        // 1 char = 3 bytes (ok)
        assert!(validate_tags(&["あ".to_string()], &config).is_ok());

        // 2 chars = 6 bytes (over 5-byte limit)
        assert!(validate_tags(&["ああ".to_string()], &config).is_err());
    }

    #[test]
    fn test_validate_tags_mixed_valid_invalid() {
        let config = small_config(); // max_tag_length = 5

        // Mix of valid and invalid tags - should fail on first invalid
        let tags = vec!["ok".to_string(), "toolong".to_string(), "x".to_string()];
        assert!(validate_tags(&tags, &config).is_err());
    }

    #[test]
    fn test_validate_tags_empty_tag() {
        let config = test_config();

        // Empty string as a tag (currently allowed by validation)
        // This tests current behaviour - may want to reject empty tags
        assert!(validate_tags(&["".to_string()], &config).is_ok());
    }
}
