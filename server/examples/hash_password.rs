// Simple utility to hash a password
// Usage: cargo run --example hash_password

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2, ParamsBuilder,
};

fn main() {
    let password = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "admin".to_string());

    // Generate random salt
    let salt = SaltString::generate(&mut OsRng);

    // Configure Argon2 parameters (match server settings)
    let params = ParamsBuilder::new()
        .m_cost(19456)
        .t_cost(2)
        .p_cost(1)
        .build()
        .expect("Failed to build Argon2 params");

    // Create Argon2 instance
    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        params,
    );

    // Hash the password
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .expect("Failed to hash password");

    let hash = password_hash.to_string();

    println!("Hash: {}", hash);
    println!();
    println!("To update admin password in database:");
    println!(
        "sqlite3 jottery.db \"UPDATE users SET password_hash='{}' WHERE email='admin@localhost';\"",
        hash
    );
}
