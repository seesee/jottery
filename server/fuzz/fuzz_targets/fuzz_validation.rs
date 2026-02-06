//! Fuzz testing for input validation functions
//!
//! Run with: cargo +nightly fuzz run fuzz_validation
//!
//! This tests that validation functions:
//! 1. Never panic on arbitrary input
//! 2. Always return a valid Result (Ok or Err)
//! 3. Handle edge cases gracefully (empty, very long, unicode, etc.)

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

use jottery_server::config::Config;
use jottery_server::utils::validation::{
    validate_device_name, validate_inbox_content, validate_note_content, validate_tags,
};

/// Arbitrary config with reasonable limits for fuzzing
fn fuzz_config() -> Config {
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
        // Use smaller limits for faster fuzzing
        max_device_name_length: 1000,
        max_inbox_content_size: 10_000,
        max_note_content_size: 50_000,
        max_tag_length: 500,
        max_tags_per_note: 100,
    }
}

/// Input structure for fuzzing validation functions
#[derive(Debug, Arbitrary)]
struct FuzzInput {
    device_name: String,
    inbox_content: String,
    note_content: String,
    tags: Vec<String>,
}

fuzz_target!(|input: FuzzInput| {
    let config = fuzz_config();

    // Test device name validation - should never panic
    let _ = validate_device_name(&input.device_name, &config);

    // Test inbox content validation - should never panic
    let _ = validate_inbox_content(&input.inbox_content, &config);

    // Test note content validation - should never panic
    let _ = validate_note_content(&input.note_content, &config);

    // Test tags validation - should never panic
    let _ = validate_tags(&input.tags, &config);
});
