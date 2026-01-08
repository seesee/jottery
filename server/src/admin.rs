//! Jottery Admin CLI tool for server management
//!
//! This tool provides command-line access to administrative functions
//! for managing users, devices, and viewing server statistics.

use clap::{Parser, Subcommand};
use comfy_table::{presets::UTF8_FULL, Cell, Color, ContentArrangement, Table};
use jottery_server::config::Config;
use jottery_server::db::{init_pool, UserRepository};
use jottery_server::utils::password::hash_password_with_params;
use sqlx::SqlitePool;

/// Device record for admin CLI (simplified from Client)
#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
struct Device {
    id: String,
    user_id: String,
    device_name: String,
    device_type: String,
    created_at: String,
    last_seen_at: Option<String>,
    is_active: Option<i64>,
}

#[derive(Parser)]
#[command(name = "jottery-admin")]
#[command(about = "Jottery server administration tool")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Database URL (overrides DATABASE_URL env var)
    #[arg(long, global = true)]
    database_url: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// User management commands
    Users {
        #[command(subcommand)]
        action: UserCommands,
    },
    /// Device management commands
    Devices {
        #[command(subcommand)]
        action: DeviceCommands,
    },
    /// Show server statistics
    Stats,
}

#[derive(Subcommand)]
enum UserCommands {
    /// List all users
    List {
        /// Show only pending users
        #[arg(long)]
        pending: bool,
        /// Show only active users
        #[arg(long)]
        active: bool,
    },
    /// Show user details
    Show {
        /// User ID or email
        user: String,
    },
    /// Approve a pending user
    Approve {
        /// User ID or email
        user: String,
    },
    /// Deactivate a user account
    Deactivate {
        /// User ID or email
        user: String,
    },
    /// Reactivate a user account
    Activate {
        /// User ID or email
        user: String,
    },
    /// Delete a user and all their data
    Delete {
        /// User ID or email
        user: String,
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Reset a user's password
    ResetPassword {
        /// User ID or email
        user: String,
        /// New password (will prompt if not provided)
        #[arg(long)]
        password: Option<String>,
    },
}

#[derive(Subcommand)]
enum DeviceCommands {
    /// List all devices
    List {
        /// Filter by user ID or email
        #[arg(long)]
        user: Option<String>,
    },
    /// Delete a device
    Delete {
        /// Device ID
        device_id: String,
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Load config and connect to database
    let config = Config::from_env()?;
    let database_url = cli.database_url.unwrap_or(config.database_url);
    let pool = init_pool(&database_url).await?;

    match cli.command {
        Commands::Users { action } => handle_users(&pool, action).await?,
        Commands::Devices { action } => handle_devices(&pool, action).await?,
        Commands::Stats => handle_stats(&pool).await?,
    }

    Ok(())
}

async fn handle_users(pool: &SqlitePool, action: UserCommands) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        UserCommands::List { pending, active } => {
            let users = UserRepository::get_all(pool).await?;

            let filtered: Vec<_> = users
                .into_iter()
                .filter(|u| {
                    if pending {
                        u.approved == 0
                    } else if active {
                        u.is_active == 1 && u.approved == 1
                    } else {
                        true
                    }
                })
                .collect();

            if filtered.is_empty() {
                println!("No users found.");
                return Ok(());
            }

            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .set_content_arrangement(ContentArrangement::Dynamic)
                .set_header(vec![
                    Cell::new("ID").fg(Color::Cyan),
                    Cell::new("Email").fg(Color::Cyan),
                    Cell::new("Status").fg(Color::Cyan),
                    Cell::new("Admin").fg(Color::Cyan),
                    Cell::new("Notes").fg(Color::Cyan),
                    Cell::new("Devices").fg(Color::Cyan),
                    Cell::new("Created").fg(Color::Cyan),
                ]);

            for user in filtered {
                let status = if user.is_active == 0 {
                    Cell::new("inactive").fg(Color::Red)
                } else if user.approved == 0 {
                    Cell::new("pending").fg(Color::Yellow)
                } else {
                    Cell::new("active").fg(Color::Green)
                };

                let admin = if user.is_admin == 1 {
                    Cell::new("yes").fg(Color::Magenta)
                } else {
                    Cell::new("no")
                };

                let note_count = UserRepository::get_note_count(pool, &user.id).await.unwrap_or(0);
                let device_count = UserRepository::get_device_count(pool, &user.id).await.unwrap_or(0);

                // Format date nicely
                let created = user.created_at.split('T').next().unwrap_or(&user.created_at);

                table.add_row(vec![
                    Cell::new(&user.id[..8]),  // Short ID
                    Cell::new(&user.email),
                    status,
                    admin,
                    Cell::new(note_count),
                    Cell::new(device_count),
                    Cell::new(created),
                ]);
            }

            println!("{table}");
        }

        UserCommands::Show { user } => {
            let user_record = find_user(pool, &user).await?;

            println!("\n  User Details");
            println!("  ────────────────────────────────────────");
            println!("  ID:           {}", user_record.id);
            println!("  Email:        {}", user_record.email);
            println!("  Status:       {}", if user_record.is_active == 0 {
                "Inactive"
            } else if user_record.approved == 0 {
                "Pending Approval"
            } else {
                "Active"
            });
            println!("  Admin:        {}", if user_record.is_admin == 1 { "Yes" } else { "No" });
            println!("  Created:      {}", user_record.created_at);

            if let Some(approved_at) = &user_record.approved_at {
                println!("  Approved:     {}", approved_at);
            }
            if let Some(last_login) = &user_record.last_login_at {
                println!("  Last Login:   {}", last_login);
            }
            println!("  Storage Quota: {} MB", user_record.storage_quota_mb.unwrap_or(1000));

            // Get statistics
            let note_count = UserRepository::get_note_count(pool, &user_record.id).await.unwrap_or(0);
            let device_count = UserRepository::get_device_count(pool, &user_record.id).await.unwrap_or(0);

            println!("\n  Statistics");
            println!("  ────────────────────────────────────────");
            println!("  Notes:        {}", note_count);
            println!("  Devices:      {}", device_count);

            // List devices
            let devices = get_devices_for_user(pool, &user_record.id).await?;
            if !devices.is_empty() {
                println!("\n  Devices");
                println!("  ────────────────────────────────────────");
                for device in devices {
                    let status = if device.is_active.unwrap_or(0) == 1 { "active" } else { "inactive" };
                    let last_seen = device.last_seen_at.as_deref().unwrap_or("never");
                    println!("  • {} ({}) - {} - last seen: {}",
                        device.device_name,
                        device.device_type,
                        status,
                        last_seen.split('T').next().unwrap_or(last_seen)
                    );
                }
            }

            println!();
        }

        UserCommands::Approve { user } => {
            let user_record = find_user(pool, &user).await?;

            if user_record.approved == 1 {
                println!("User {} is already approved.", user_record.email);
                return Ok(());
            }

            UserRepository::approve(pool, &user_record.id, "admin-cli").await?;
            println!("✓ Approved user: {}", user_record.email);
        }

        UserCommands::Deactivate { user } => {
            let user_record = find_user(pool, &user).await?;

            if user_record.is_active == 0 {
                println!("User {} is already inactive.", user_record.email);
                return Ok(());
            }

            if user_record.is_admin == 1 {
                eprintln!("Error: Cannot deactivate admin user via CLI.");
                std::process::exit(1);
            }

            UserRepository::deactivate(pool, &user_record.id).await?;
            println!("✓ Deactivated user: {}", user_record.email);
        }

        UserCommands::Activate { user } => {
            let user_record = find_user(pool, &user).await?;

            if user_record.is_active == 1 {
                println!("User {} is already active.", user_record.email);
                return Ok(());
            }

            UserRepository::activate(pool, &user_record.id).await?;
            println!("✓ Activated user: {}", user_record.email);
        }

        UserCommands::Delete { user, yes } => {
            let user_record = find_user(pool, &user).await?;

            if user_record.is_admin == 1 {
                eprintln!("Error: Cannot delete admin user via CLI.");
                std::process::exit(1);
            }

            if !yes {
                println!("This will permanently delete user '{}' and all their data.", user_record.email);
                println!("This action cannot be undone.");
                print!("Are you sure? [y/N] ");
                use std::io::{self, Write};
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;

                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Aborted.");
                    return Ok(());
                }
            }

            // Delete user's notes
            sqlx::query!("DELETE FROM notes WHERE user_id = ?", user_record.id)
                .execute(pool)
                .await?;

            // Delete user's devices
            sqlx::query!("DELETE FROM clients WHERE user_id = ?", user_record.id)
                .execute(pool)
                .await?;

            // Delete user
            UserRepository::delete(pool, &user_record.id).await?;

            println!("✓ Deleted user: {}", user_record.email);
        }

        UserCommands::ResetPassword { user, password } => {
            let user_record = find_user(pool, &user).await?;

            // Get password (from argument or prompt)
            let new_password = if let Some(pwd) = password {
                pwd
            } else {
                use std::io::{self, Write};

                print!("Enter new password for {}: ", user_record.email);
                io::stdout().flush()?;

                // Read password (note: not hidden, consider using rpassword crate for production)
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                let pwd = input.trim().to_string();

                if pwd.is_empty() {
                    eprintln!("Error: Password cannot be empty.");
                    std::process::exit(1);
                }

                pwd
            };

            // Validate password length
            if new_password.len() < 12 {
                eprintln!("Error: Password must be at least 12 characters.");
                std::process::exit(1);
            }

            // Load config for Argon2 parameters
            let config = Config::from_env()?;

            // Hash the new password
            let password_hash = hash_password_with_params(
                &new_password,
                config.argon2_m_cost,
                config.argon2_t_cost,
                config.argon2_p_cost,
            ).map_err(|e| format!("Failed to hash password: {}", e))?;

            // Update in database
            sqlx::query!(
                "UPDATE users SET password_hash = ? WHERE id = ?",
                password_hash,
                user_record.id
            )
            .execute(pool)
            .await?;

            println!("✓ Password reset for user: {}", user_record.email);
        }
    }

    Ok(())
}

async fn handle_devices(pool: &SqlitePool, action: DeviceCommands) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        DeviceCommands::List { user } => {
            let devices = if let Some(user_filter) = user {
                let user_record = find_user(pool, &user_filter).await?;
                get_devices_for_user(pool, &user_record.id).await?
            } else {
                get_all_devices(pool).await?
            };

            if devices.is_empty() {
                println!("No devices found.");
                return Ok(());
            }

            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .set_content_arrangement(ContentArrangement::Dynamic)
                .set_header(vec![
                    Cell::new("ID").fg(Color::Cyan),
                    Cell::new("User").fg(Color::Cyan),
                    Cell::new("Name").fg(Color::Cyan),
                    Cell::new("Type").fg(Color::Cyan),
                    Cell::new("Status").fg(Color::Cyan),
                    Cell::new("Last Seen").fg(Color::Cyan),
                ]);

            for device in devices {
                let status = if device.is_active.unwrap_or(0) == 1 {
                    Cell::new("active").fg(Color::Green)
                } else {
                    Cell::new("inactive").fg(Color::Red)
                };

                let last_seen = device.last_seen_at
                    .as_ref()
                    .map(|s| s.split('T').next().unwrap_or(s).to_string())
                    .unwrap_or_else(|| "never".to_string());

                // Get user email
                let user_email = sqlx::query_scalar!(
                    "SELECT email FROM users WHERE id = ?",
                    device.user_id
                )
                .fetch_optional(pool)
                .await?
                .unwrap_or_else(|| "unknown".to_string());

                table.add_row(vec![
                    Cell::new(&device.id[..8]),
                    Cell::new(user_email),
                    Cell::new(&device.device_name),
                    Cell::new(&device.device_type),
                    status,
                    Cell::new(last_seen),
                ]);
            }

            println!("{table}");
        }

        DeviceCommands::Delete { device_id, yes } => {
            // Find device
            let id_prefix = format!("{}%", device_id);
            let device = sqlx::query_as!(
                Device,
                "SELECT id, user_id, device_name, device_type, created_at, last_seen_at, is_active FROM clients WHERE id = ? OR id LIKE ?",
                device_id,
                id_prefix
            )
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| format!("Device not found: {}", device_id))?;

            if !yes {
                println!("This will delete device '{}' ({}).", device.device_name, device.device_type);
                print!("Are you sure? [y/N] ");
                use std::io::{self, Write};
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;

                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Aborted.");
                    return Ok(());
                }
            }

            sqlx::query!("DELETE FROM clients WHERE id = ?", device.id)
                .execute(pool)
                .await?;

            println!("✓ Deleted device: {} ({})", device.device_name, device.device_type);
        }
    }

    Ok(())
}

async fn handle_stats(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    // Get user counts
    let (total_users, approved_users, pending_users) = UserRepository::get_count_by_status(pool).await?;
    let inactive_users: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM users WHERE is_active = 0")
        .fetch_one(pool)
        .await?
        .into();

    // Get device count
    let total_devices: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM clients")
        .fetch_one(pool)
        .await?
        .into();

    let active_devices: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM clients WHERE is_active = 1")
        .fetch_one(pool)
        .await?
        .into();

    // Get note count
    let total_notes: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM notes WHERE deleted = 0")
        .fetch_one(pool)
        .await?
        .into();

    let deleted_notes: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM notes WHERE deleted = 1")
        .fetch_one(pool)
        .await?
        .into();

    // Get attachment count and size
    let total_attachments: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM attachments_meta")
        .fetch_one(pool)
        .await?
        .into();

    let total_storage: i64 = sqlx::query_scalar!(
        "SELECT COALESCE(SUM(size), 0) FROM attachments_meta"
    )
    .fetch_one(pool)
    .await?;

    // Get version count
    let total_versions: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM note_versions")
        .fetch_one(pool)
        .await?
        .into();

    println!("\n  Jottery Server Statistics");
    println!("  ════════════════════════════════════════");

    println!("\n  Users");
    println!("  ────────────────────────────────────────");
    println!("  Total:          {}", total_users);
    println!("  Approved:       {}", approved_users);
    println!("  Pending:        {}", pending_users);
    println!("  Inactive:       {}", inactive_users);

    println!("\n  Devices");
    println!("  ────────────────────────────────────────");
    println!("  Total:          {}", total_devices);
    println!("  Active:         {}", active_devices);

    println!("\n  Notes");
    println!("  ────────────────────────────────────────");
    println!("  Active:         {}", total_notes);
    println!("  Deleted:        {}", deleted_notes);
    println!("  Versions:       {}", total_versions);

    println!("\n  Storage");
    println!("  ────────────────────────────────────────");
    println!("  Attachments:    {}", total_attachments);
    println!("  Total Size:     {}", format_bytes(total_storage as u64));

    println!();

    Ok(())
}

/// Find user by ID (partial match) or email
async fn find_user(pool: &SqlitePool, identifier: &str) -> Result<jottery_server::models::User, Box<dyn std::error::Error>> {
    // Try exact email match first
    if identifier.contains('@') {
        if let Ok(user) = UserRepository::get_by_email(pool, identifier).await {
            return Ok(user);
        }
    }

    // Try ID match (exact or prefix)
    let id_prefix = format!("{}%", identifier);
    let user = sqlx::query_as!(
        jottery_server::models::User,
        "SELECT * FROM users WHERE id = ? OR id LIKE ?",
        identifier,
        id_prefix
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| format!("User not found: {}", identifier))?;

    Ok(user)
}

/// Get all devices for a user
async fn get_devices_for_user(pool: &SqlitePool, user_id: &str) -> Result<Vec<Device>, sqlx::Error> {
    sqlx::query_as!(
        Device,
        "SELECT id, user_id, device_name, device_type, created_at, last_seen_at, is_active FROM clients WHERE user_id = ? ORDER BY created_at DESC",
        user_id
    )
    .fetch_all(pool)
    .await
}

/// Get all devices
async fn get_all_devices(pool: &SqlitePool) -> Result<Vec<Device>, sqlx::Error> {
    sqlx::query_as!(
        Device,
        "SELECT id, user_id, device_name, device_type, created_at, last_seen_at, is_active FROM clients ORDER BY created_at DESC"
    )
    .fetch_all(pool)
    .await
}

/// Format bytes to human-readable string
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}
