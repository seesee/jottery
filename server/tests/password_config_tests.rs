// Password configuration tests
//
// Tests for configurable Argon2 password hashing parameters

use jottery_server::utils::password::{hash_password_with_params, verify_password};

#[test]
fn test_hash_password_with_default_params() {
    let password = "test_password_123";
    let hash = hash_password_with_params(password, 19456, 2, 1)
        .expect("Failed to hash password");

    // Verify the hash works
    assert!(
        verify_password(password, &hash).expect("Failed to verify"),
        "Password should verify with correct password"
    );

    // Verify incorrect password fails
    assert!(
        !verify_password("wrong_password", &hash).expect("Failed to verify"),
        "Wrong password should not verify"
    );

    // Verify hash format
    assert!(
        hash.starts_with("$argon2id$"),
        "Hash should use Argon2id algorithm"
    );
}

#[test]
fn test_hash_password_with_low_security_params() {
    // Minimum recommended parameters (fast but less secure)
    let password = "test_password";
    let hash = hash_password_with_params(password, 19456, 2, 1)
        .expect("Failed to hash with low security params");

    assert!(verify_password(password, &hash).expect("Failed to verify"));
}

#[test]
fn test_hash_password_with_medium_security_params() {
    // Medium security parameters (balanced)
    let password = "test_password";
    let hash = hash_password_with_params(password, 65536, 3, 1)
        .expect("Failed to hash with medium security params");

    assert!(verify_password(password, &hash).expect("Failed to verify"));

    // Verify hash contains expected parameters
    assert!(hash.contains("m=65536"), "Hash should contain m=65536");
    assert!(hash.contains("t=3"), "Hash should contain t=3");
}

#[test]
fn test_hash_password_with_high_security_params() {
    // High security parameters (slow but very secure)
    let password = "test_password";
    let hash = hash_password_with_params(password, 262144, 4, 1)
        .expect("Failed to hash with high security params");

    assert!(verify_password(password, &hash).expect("Failed to verify"));

    // Verify hash contains expected parameters
    assert!(hash.contains("m=262144"), "Hash should contain m=262144");
    assert!(hash.contains("t=4"), "Hash should contain t=4");
}

#[test]
fn test_hash_password_with_parallel_params() {
    // Test parallelism parameter
    let password = "test_password";

    // Single thread
    let hash1 = hash_password_with_params(password, 19456, 2, 1)
        .expect("Failed to hash with p=1");
    assert!(verify_password(password, &hash1).expect("Failed to verify"));

    // Multiple threads
    let hash2 = hash_password_with_params(password, 19456, 2, 4)
        .expect("Failed to hash with p=4");
    assert!(verify_password(password, &hash2).expect("Failed to verify"));

    // Hashes should be different (different salts)
    assert_ne!(hash1, hash2, "Different salts should produce different hashes");
}

#[test]
fn test_hash_password_params_uniqueness() {
    // Same password with same parameters should produce different hashes (different salts)
    let password = "same_password";

    let hash1 = hash_password_with_params(password, 19456, 2, 1)
        .expect("Failed to hash");
    let hash2 = hash_password_with_params(password, 19456, 2, 1)
        .expect("Failed to hash");

    assert_ne!(hash1, hash2, "Same password should produce different hashes due to unique salts");

    // Both should verify correctly
    assert!(verify_password(password, &hash1).expect("Failed to verify hash1"));
    assert!(verify_password(password, &hash2).expect("Failed to verify hash2"));
}

#[test]
fn test_hash_password_cross_param_verification() {
    // Hash created with one set of parameters should verify correctly
    // (verification reads parameters from the hash itself)
    let password = "test_password";

    let hash_low = hash_password_with_params(password, 19456, 2, 1)
        .expect("Failed to hash with low params");
    let hash_high = hash_password_with_params(password, 65536, 3, 1)
        .expect("Failed to hash with high params");

    // Both hashes should verify correctly regardless of parameters used to create them
    assert!(verify_password(password, &hash_low).expect("Failed to verify low security hash"));
    assert!(verify_password(password, &hash_high).expect("Failed to verify high security hash"));
}

#[test]
fn test_hash_password_minimum_params() {
    // Test with minimal valid parameters
    let password = "test_password";
    let hash = hash_password_with_params(password, 8192, 1, 1)
        .expect("Failed to hash with minimum params");

    assert!(verify_password(password, &hash).expect("Failed to verify"));
}

#[test]
fn test_hash_password_empty_password() {
    // Empty password should still hash successfully
    let password = "";
    let hash = hash_password_with_params(password, 19456, 2, 1)
        .expect("Failed to hash empty password");

    assert!(verify_password(password, &hash).expect("Failed to verify empty password"));
    assert!(!verify_password("not_empty", &hash).expect("Failed to verify"));
}

#[test]
fn test_hash_password_long_password() {
    // Very long password (1000 characters)
    let password = "a".repeat(1000);
    let hash = hash_password_with_params(&password, 19456, 2, 1)
        .expect("Failed to hash long password");

    assert!(verify_password(&password, &hash).expect("Failed to verify long password"));
}

#[test]
fn test_hash_password_special_characters() {
    // Password with special characters
    let password = "P@ssw0rd!#$%^&*()_+-=[]{}|;':\",./<>?`~";
    let hash = hash_password_with_params(password, 19456, 2, 1)
        .expect("Failed to hash password with special chars");

    assert!(verify_password(password, &hash).expect("Failed to verify"));
}

#[test]
fn test_hash_password_unicode() {
    // Password with Unicode characters
    let password = "パスワード🔒密碼";
    let hash = hash_password_with_params(password, 19456, 2, 1)
        .expect("Failed to hash Unicode password");

    assert!(verify_password(password, &hash).expect("Failed to verify Unicode password"));
}

#[cfg(test)]
mod performance_documentation {
    use super::*;

    #[test]
    fn document_performance_tradeoffs() {
        // Document the approximate performance characteristics
        // (Note: Actual timings vary by hardware)

        println!("\n=== Argon2id Parameter Performance Characteristics ===");
        println!("\nLow Security (m=19456, t=2, p=1):");
        println!("  - Memory: ~19 MiB");
        println!("  - Approximate time: ~50ms on modern CPU");
        println!("  - Use case: Development, low-risk applications");

        println!("\nMedium Security (m=65536, t=3, p=1):");
        println!("  - Memory: ~64 MiB");
        println!("  - Approximate time: ~200ms on modern CPU");
        println!("  - Use case: Standard production deployments");

        println!("\nHigh Security (m=262144, t=4, p=1):");
        println!("  - Memory: ~256 MiB");
        println!("  - Approximate time: ~1s on modern CPU");
        println!("  - Use case: High-security applications, sensitive data");

        println!("\n=== Security Considerations ===");
        println!("- Higher memory cost (m) increases resistance to GPU/ASIC attacks");
        println!("- Higher time cost (t) increases resistance to brute-force attacks");
        println!("- Parallelism (p) should match server CPU cores for optimal performance");
        println!("- Balance security needs with user experience (login time)");
        println!("====================================================\n");

        // Verify we can hash with all documented parameter sets
        let password = "test";

        let low = hash_password_with_params(password, 19456, 2, 1);
        assert!(low.is_ok(), "Low security params should work");

        let medium = hash_password_with_params(password, 65536, 3, 1);
        assert!(medium.is_ok(), "Medium security params should work");

        let high = hash_password_with_params(password, 262144, 4, 1);
        assert!(high.is_ok(), "High security params should work");
    }

    #[test]
    fn document_recommended_production_settings() {
        println!("\n=== Recommended Production Settings ===");
        println!("\nFor most production deployments:");
        println!("  ARGON2_M_COST=65536   # 64 MiB memory");
        println!("  ARGON2_T_COST=3       # 3 iterations");
        println!("  ARGON2_P_COST=1       # Single thread");
        println!("\nFor high-security deployments:");
        println!("  ARGON2_M_COST=262144  # 256 MiB memory");
        println!("  ARGON2_T_COST=4       # 4 iterations");
        println!("  ARGON2_P_COST=1       # Single thread");
        println!("\nAdjust based on:");
        println!("  - Available server memory");
        println!("  - Acceptable login latency");
        println!("  - Threat model requirements");
        println!("==========================================\n");
    }
}
