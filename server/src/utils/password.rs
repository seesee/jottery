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
}
