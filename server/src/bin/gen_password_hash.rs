/// Simple utility to generate Argon2id password hash
/// Usage: cargo run --bin gen_password_hash

use jottery_server::utils::password::hash_password;

fn main() {
    let password = "changeme";
    match hash_password(password) {
        Ok(hash) => {
            // Note: Password is "changeme" (hardcoded above)
            println!("Hash: {}", hash);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
