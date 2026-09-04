use keyring::Entry;

fn main() {
    println!("Testing keyring...");

    let entry = match Entry::new("jottery-tui", "database-password") {
        Ok(e) => {
            println!("Entry created successfully");
            e
        }
        Err(e) => {
            println!("Failed to create entry: {:?}", e);
            return;
        }
    };

    // Check current state
    println!("\n1. Checking if password exists...");
    match entry.get_password() {
        Ok(pwd) => println!("   Found existing password: {} chars", pwd.len()),
        Err(keyring::Error::NoEntry) => println!("   No password stored"),
        Err(e) => println!("   Error: {:?}", e),
    }

    // Store password
    println!("\n2. Storing password 'testpwd123'...");
    match entry.set_password("testpwd123") {
        Ok(()) => println!("   Store returned Ok"),
        Err(e) => println!("   Store error: {:?}", e),
    }

    // Retrieve password
    println!("\n3. Retrieving password...");
    match entry.get_password() {
        Ok(pwd) => println!("   Retrieved {} bytes; matches stored value: {}", pwd.len(), pwd == "testpwd123"),
        Err(e) => println!("   Retrieve error: {:?}", e),
    }

    // Check with system command
    println!("\n4. Checking with macOS security command...");
    let output = std::process::Command::new("security")
        .args(["find-generic-password", "-s", "jottery-tui", "-a", "database-password"])
        .output();
    match output {
        Ok(out) => {
            if out.status.success() {
                println!("   Found in keychain!");
                println!("   {}", String::from_utf8_lossy(&out.stdout));
            } else {
                println!("   Not found: {}", String::from_utf8_lossy(&out.stderr));
            }
        }
        Err(e) => println!("   Command failed: {}", e),
    }

    // Clean up
    println!("\n5. Deleting password...");
    match entry.delete_credential() {
        Ok(()) => println!("   Deleted"),
        Err(e) => println!("   Delete error: {:?}", e),
    }
}
