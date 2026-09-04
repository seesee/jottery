use keyring::Entry;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let entry = Entry::new("jottery-tui", "database-password").unwrap();

    if args.len() > 1 && args[1] == "store" {
        println!("Storing password...");
        entry.set_password("persisttest").unwrap();
        println!("Stored! Run without 'store' arg to retrieve.");
    } else if args.len() > 1 && args[1] == "delete" {
        println!("Deleting...");
        let _ = entry.delete_credential();
        println!("Deleted.");
    } else {
        println!("Retrieving password...");
        match entry.get_password() {
            Ok(pwd) => println!("Found entry ({} bytes)", pwd.len()),
            Err(keyring::Error::NoEntry) => println!("No entry found!"),
            Err(e) => println!("Error: {:?}", e),
        }
    }
}
