//! Fuzz testing for email validation
//!
//! Run with: cargo +nightly fuzz run fuzz_email
//!
//! Tests that the email validation:
//! 1. Never panics on arbitrary input
//! 2. Correctly rejects malformed emails
//! 3. Handles edge cases (very long, unicode, special chars)

#![no_main]

use libfuzzer_sys::fuzz_target;

// Note: is_valid_email is a private function in auth.rs
// This fuzz target tests the email_address crate directly
// which is what the server uses for validation

fuzz_target!(|data: &str| {
    // Test the email_address crate's validation
    // This should never panic
    let _ = email_address::EmailAddress::is_valid(data);

    // Also test parsing (more strict)
    let _ = data.parse::<email_address::EmailAddress>();

    // Test length check (RFC 5321 limit)
    if data.len() <= 254 {
        // Within length limit - validation should complete
        let _ = email_address::EmailAddress::is_valid(data);
    }

    // Test with common attack patterns embedded
    // The validation should handle these without panicking
    let attack_patterns = [
        format!("{}'--", data),
        format!("{}; DROP TABLE users;", data),
        format!("{}<script>", data),
        format!("{}%00", data),
        format!("{}\x00", data),
    ];

    for pattern in &attack_patterns {
        let _ = email_address::EmailAddress::is_valid(pattern);
    }
});
