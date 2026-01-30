use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2, ParamsBuilder,
};

/// Password hashing error
#[derive(Debug)]
pub enum PasswordError {
    HashError(String),
    VerifyError(String),
}

impl std::fmt::Display for PasswordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PasswordError::HashError(msg) => write!(f, "Password hash error: {}", msg),
            PasswordError::VerifyError(msg) => write!(f, "Password verify error: {}", msg),
        }
    }
}

impl std::error::Error for PasswordError {}

/// Validate password strength based on complexity level
///
/// # Complexity Levels
/// - "none": Only minimum length (12 characters)
/// - "basic": Length + at least 2 of 4 character classes
/// - "standard": Length + at least 3 of 4 character classes (default)
/// - "strong": Length + all 4 character classes
///
/// Character classes: uppercase, lowercase, digit, special
pub fn validate_password_strength(password: &str, complexity: &str) -> Result<(), String> {
    const MIN_LENGTH: usize = 12;

    // Always check minimum length
    if password.len() < MIN_LENGTH {
        return Err(format!(
            "Password must be at least {} characters",
            MIN_LENGTH
        ));
    }

    // If complexity is "none", only length matters
    if complexity == "none" {
        return Ok(());
    }

    // Count character classes present
    let has_uppercase = password.chars().any(|c| c.is_ascii_uppercase());
    let has_lowercase = password.chars().any(|c| c.is_ascii_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_special = password.chars().any(|c| !c.is_alphanumeric());

    let class_count = [has_uppercase, has_lowercase, has_digit, has_special]
        .iter()
        .filter(|&&x| x)
        .count();

    let required_classes = match complexity {
        "basic" => 2,
        "strong" => 4,
        _ => 3, // "standard" is default
    };

    if class_count < required_classes {
        let classes_needed: Vec<&str> = [
            (!has_uppercase).then_some("uppercase letter"),
            (!has_lowercase).then_some("lowercase letter"),
            (!has_digit).then_some("number"),
            (!has_special).then_some("special character"),
        ]
        .into_iter()
        .flatten()
        .collect();

        return Err(format!(
            "Password requires at least {} of 4 character types. Missing: {}",
            required_classes,
            classes_needed.join(", ")
        ));
    }

    Ok(())
}

/// Hash a password using Argon2id with custom parameters
///
/// # Parameters
/// - `password`: The password to hash
/// - `m_cost`: Memory cost in KiB (e.g., 19456 = 19 MiB)
/// - `t_cost`: Time cost / iterations (e.g., 2)
/// - `p_cost`: Parallelism / threads (e.g., 1)
///
/// These parameters provide strong security while being reasonable for server-side auth.
pub fn hash_password_with_params(
    password: &str,
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
) -> Result<String, PasswordError> {
    // Generate random salt
    let salt = SaltString::generate(&mut OsRng);

    // Configure Argon2 parameters
    let params = ParamsBuilder::new()
        .m_cost(m_cost)
        .t_cost(t_cost)
        .p_cost(p_cost)
        .build()
        .map_err(|e| PasswordError::HashError(e.to_string()))?;

    // Create Argon2 instance with our parameters
    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        params,
    );

    // Hash the password
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| PasswordError::HashError(e.to_string()))?;

    Ok(password_hash.to_string())
}

/// Hash a password using Argon2id with default parameters
///
/// Uses Argon2id with:
/// - Memory cost: 19456 KiB (19 MiB)
/// - Time cost: 2 iterations
/// - Parallelism: 1 thread
///
/// For custom parameters, use `hash_password_with_params`.
#[allow(dead_code)]
pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    hash_password_with_params(password, 19456, 2, 1)
}

/// Verify a password against its hash
///
/// Uses constant-time comparison to prevent timing attacks.
pub fn verify_password(password: &str, hash: &str) -> Result<bool, PasswordError> {
    // Parse the stored hash
    let parsed_hash =
        PasswordHash::new(hash).map_err(|e| PasswordError::VerifyError(e.to_string()))?;

    // Create Argon2 instance (parameters come from the hash)
    let argon2 = Argon2::default();

    // Verify the password (constant-time comparison)
    match argon2.verify_password(password.as_bytes(), &parsed_hash) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(PasswordError::VerifyError(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify() {
        let password = "test_password_123";
        let hash = hash_password(password).expect("Failed to hash password");

        // Correct password should verify
        assert!(
            verify_password(password, &hash).expect("Failed to verify"),
            "Password verification failed"
        );

        // Incorrect password should not verify
        assert!(
            !verify_password("wrong_password", &hash).expect("Failed to verify"),
            "Wrong password incorrectly verified"
        );
    }

    #[test]
    fn test_unique_hashes() {
        let password = "same_password";
        let hash1 = hash_password(password).expect("Failed to hash");
        let hash2 = hash_password(password).expect("Failed to hash");

        // Same password should produce different hashes (due to unique salts)
        assert_ne!(hash1, hash2, "Hashes should be unique");

        // But both should verify correctly
        assert!(
            verify_password(password, &hash1).expect("Failed to verify"),
            "First hash failed"
        );
        assert!(
            verify_password(password, &hash2).expect("Failed to verify"),
            "Second hash failed"
        );
    }

    #[test]
    fn test_verify_invalid_hash() {
        let result = verify_password("password", "invalid_hash_format");
        assert!(result.is_err(), "Should error on invalid hash format");
    }

    #[test]
    fn test_password_strength_none() {
        // "none" only checks length
        assert!(validate_password_strength("aaaaaaaaaaaa", "none").is_ok());
        assert!(validate_password_strength("short", "none").is_err());
    }

    #[test]
    fn test_password_strength_basic() {
        // "basic" needs 2 of 4 classes
        assert!(validate_password_strength("aaaaaaaaaaaA", "basic").is_ok()); // lower + upper
        assert!(validate_password_strength("aaaaaaaaaaaa", "basic").is_err()); // only lower
        assert!(validate_password_strength("aaaaaaaaaaa1", "basic").is_ok()); // lower + digit
    }

    #[test]
    fn test_password_strength_standard() {
        // "standard" needs 3 of 4 classes
        assert!(validate_password_strength("aaaaaaaaAA1", "standard").is_err()); // too short
        assert!(validate_password_strength("aaaaaaaaaAA1", "standard").is_ok()); // lower + upper + digit
        assert!(validate_password_strength("aaaaaaaaaaaa", "standard").is_err()); // only lower
        assert!(validate_password_strength("aaaaaaaaaa!A", "standard").is_ok()); // lower + upper + special
    }

    #[test]
    fn test_password_strength_strong() {
        // "strong" needs all 4 classes
        assert!(validate_password_strength("aaaaaaAA11!!", "strong").is_ok()); // all 4
        assert!(validate_password_strength("aaaaaaaaaAA1", "strong").is_err()); // missing special
        assert!(validate_password_strength("aaaaaaaaaa!1", "strong").is_err()); // missing upper
    }
}
