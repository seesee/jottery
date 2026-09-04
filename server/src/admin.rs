//! Jottery Admin CLI tool for server management
//!
//! This tool provides command-line access to administrative functions
//! for managing users, devices, and viewing server statistics.

use clap::{Parser, Subcommand};
use comfy_table::{presets::UTF8_FULL, Cell, Color, ContentArrangement, Table};
use jottery_server::config::Config;
use jottery_server::db::init_pool;
use jottery_server::services::{AdminService, UserListFilter, format_bytes};

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
    let database_url = cli.database_url.unwrap_or(config.database_url.clone());
    let pool = init_pool(&database_url).await?;

    match cli.command {
        Commands::Users { action } => handle_users(&pool, action, &config).await?,
        Commands::Devices { action } => handle_devices(&pool, action).await?,
        Commands::Stats => handle_stats(&pool).await?,
    }

    Ok(())
}

async fn handle_users(
    pool: &sqlx::SqlitePool,
    action: UserCommands,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        UserCommands::List { pending, active } => {
            let filter = UserListFilter {
                pending_only: pending,
                active_only: active,
            };
            let users = AdminService::list_users(pool, filter).await?;

            if users.is_empty() {
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

            for user in users {
                let info = AdminService::get_user_info(pool, &user.id).await?;

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

                let created = user.created_at.split('T').next().unwrap_or(&user.created_at);

                table.add_row(vec![
                    Cell::new(&user.id[..8]),
                    Cell::new(&user.email),
                    status,
                    admin,
                    Cell::new(info.note_count),
                    Cell::new(info.device_count),
                    Cell::new(created),
                ]);
            }

            println!("{table}");
        }

        UserCommands::Show { user } => {
            let info = AdminService::get_user_info(pool, &user).await?;

            println!("\n  User Details");
            println!("  ────────────────────────────────────────");
            println!("  ID:           {}", info.user.id);
            println!("  Email:        {}", info.user.email);
            println!(
                "  Status:       {}",
                if info.user.is_active == 0 {
                    "Inactive"
                } else if info.user.approved == 0 {
                    "Pending Approval"
                } else {
                    "Active"
                }
            );
            println!(
                "  Admin:        {}",
                if info.user.is_admin == 1 { "Yes" } else { "No" }
            );
            println!("  Created:      {}", info.user.created_at);

            if let Some(approved_at) = &info.user.approved_at {
                println!("  Approved:     {}", approved_at);
            }
            if let Some(last_login) = &info.user.last_login_at {
                println!("  Last Login:   {}", last_login);
            }
            println!(
                "  Storage Quota: {} MB",
                info.user.storage_quota_mb.unwrap_or(1000)
            );

            println!("\n  Statistics");
            println!("  ────────────────────────────────────────");
            println!("  Notes:        {}", info.note_count);
            println!("  Devices:      {}", info.device_count);

            if !info.devices.is_empty() {
                println!("\n  Devices");
                println!("  ────────────────────────────────────────");
                for device in info.devices {
                    let status = if device.is_active { "active" } else { "inactive" };
                    let last_seen = device.last_seen_at.as_deref().unwrap_or("never");
                    println!(
                        "  • {} ({}) - {} - last seen: {}",
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
            match AdminService::approve_user(pool, &user).await {
                Ok(u) => println!("✓ Approved user: {}", u.email),
                Err(jottery_server::services::AdminError::AlreadyInState(_)) => {
                    println!("User is already approved.");
                }
                Err(e) => return Err(e.into()),
            }
        }

        UserCommands::Deactivate { user } => {
            match AdminService::deactivate_user(pool, &user).await {
                Ok(u) => println!("✓ Deactivated user: {}", u.email),
                Err(jottery_server::services::AdminError::AlreadyInState(_)) => {
                    println!("User is already inactive.");
                }
                Err(jottery_server::services::AdminError::CannotModifyAdmin) => {
                    eprintln!("Error: Cannot deactivate admin user via CLI.");
                    std::process::exit(1);
                }
                Err(e) => return Err(e.into()),
            }
        }

        UserCommands::Activate { user } => {
            match AdminService::activate_user(pool, &user).await {
                Ok(u) => println!("✓ Activated user: {}", u.email),
                Err(jottery_server::services::AdminError::AlreadyInState(_)) => {
                    println!("User is already active.");
                }
                Err(e) => return Err(e.into()),
            }
        }

        UserCommands::Delete { user, yes } => {
            let info = AdminService::get_user_info(pool, &user).await?;

            if info.user.is_admin == 1 {
                eprintln!("Error: Cannot delete admin user via CLI.");
                std::process::exit(1);
            }

            if !yes {
                println!(
                    "This will permanently delete user '{}' and all their data.",
                    info.user.email
                );
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

            let deleted = AdminService::delete_user(pool, &info.user.id).await?;
            println!("✓ Deleted user: {}", deleted.email);
        }

        UserCommands::ResetPassword { user, password } => {
            let info = AdminService::get_user_info(pool, &user).await?;

            let new_password = if let Some(pwd) = password {
                pwd
            } else {
                use std::io::{self, Write};

                print!("Enter new password for {}: ", info.user.email);
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                let pwd = input.trim().to_string();

                if pwd.is_empty() {
                    eprintln!("Error: Password cannot be empty.");
                    std::process::exit(1);
                }

                pwd
            };

            let argon2_params = (config.argon2_m_cost, config.argon2_t_cost, config.argon2_p_cost);

            match AdminService::reset_password(pool, &info.user.id, &new_password, argon2_params)
                .await
            {
                Ok(_) => println!("✓ Password reset for user: {}", info.user.email),
                Err(jottery_server::services::AdminError::PasswordTooShort) => {
                    eprintln!("Error: Password must be at least 12 characters.");
                    std::process::exit(1);
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    Ok(())
}

async fn handle_devices(
    pool: &sqlx::SqlitePool,
    action: DeviceCommands,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        DeviceCommands::List { user } => {
            let devices = AdminService::list_devices(pool, user.as_deref()).await?;

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
                let status = if device.is_active {
                    Cell::new("active").fg(Color::Green)
                } else {
                    Cell::new("inactive").fg(Color::Red)
                };

                let last_seen = device
                    .last_seen_at
                    .as_ref()
                    .map(|s| s.split('T').next().unwrap_or(s).to_string())
                    .unwrap_or_else(|| "never".to_string());

                let user_email = AdminService::get_user_email_for_device(pool, &device.user_id)
                    .await
                    .unwrap_or_else(|_| "unknown".to_string());

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
            let devices = AdminService::list_devices(pool, None).await?;
            let device = devices
                .into_iter()
                .find(|d| d.id == device_id || d.id.starts_with(&device_id))
                .ok_or_else(|| format!("Device not found: {}", device_id))?;

            if !yes {
                println!(
                    "This will delete device '{}' ({}).",
                    device.device_name, device.device_type
                );
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

            let deleted = AdminService::delete_device(pool, &device.id).await?;
            println!(
                "✓ Deleted device: {} ({})",
                deleted.device_name, deleted.device_type
            );
        }
    }

    Ok(())
}

async fn handle_stats(pool: &sqlx::SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let stats = AdminService::get_stats(pool).await?;

    println!("\n  Jottery Server Statistics");
    println!("  ════════════════════════════════════════");

    println!("\n  Users");
    println!("  ────────────────────────────────────────");
    println!("  Total:          {}", stats.total_users);
    println!("  Approved:       {}", stats.approved_users);
    println!("  Pending:        {}", stats.pending_users);
    println!("  Inactive:       {}", stats.inactive_users);

    println!("\n  Devices");
    println!("  ────────────────────────────────────────");
    println!("  Total:          {}", stats.total_devices);
    println!("  Active:         {}", stats.active_devices);

    println!("\n  Notes");
    println!("  ────────────────────────────────────────");
    println!("  Active:         {}", stats.total_notes);
    println!("  Deleted:        {}", stats.deleted_notes);
    println!("  Versions:       {}", stats.total_versions);

    println!("\n  Storage");
    println!("  ────────────────────────────────────────");
    println!("  Attachments:    {}", stats.total_attachments);
    println!(
        "  Total Size:     {}",
        format_bytes(stats.total_storage_bytes as u64)
    );

    println!();

    Ok(())
}
