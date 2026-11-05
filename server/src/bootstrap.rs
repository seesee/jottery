//! First-run handling for the seeded admin account.
//!
//! Migration 003 seeds `admin@localhost` with a password that is published in
//! the migration itself, and existing deployments may still be using it. This
//! runs after migrations on every start and makes sure that:
//!
//! - a fresh install never starts with a known password,
//! - `DEFAULT_ADMIN_EMAIL` and `DEFAULT_ADMIN_PASSWORD` are honoured,
//! - an existing install still on the default keeps working, so nobody is
//!   locked out, but its admin can do nothing until the password is changed.

use rand::{distributions::Alphanumeric, Rng};
use sqlx::SqlitePool;

use crate::{
    config::Config,
    utils::password::{hash_password_with_params, validate_password_strength},
};

/// The admin row inserted by migration 003.
pub const SEED_ADMIN_ID: &str = "00000000-0000-0000-0000-000000000001";

/// The hash migration 003 seeds. Comparing against it, rather than verifying
/// the password, identifies an untouched account without an Argon2 run.
const SEED_ADMIN_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$EVQp6JMJNY3JYI2sPBp58Q$ooV7ZZOE7PuFFrJA60P0cYLiQH3k7CdBMLaF5bakVUA";

const GENERATED_PASSWORD_LENGTH: usize = 32;

/// What `ensure_default_admin` did. The generated password is returned so tests
/// can sign in with it; in production it is only ever seen in the log.
#[derive(Debug, PartialEq, Eq)]
pub enum AdminBootstrap {
    /// The seeded row is gone: an operator removed it on purpose.
    NoSeedAccount,
    /// The account already has an operator-chosen password. Nothing to do.
    AlreadyConfigured,
    /// Credentials from the environment were applied.
    AppliedConfigured,
    /// A fresh install with no configured password: a random one was set.
    GeneratedPassword(String),
    /// An existing install still on the default. Its login keeps working but
    /// the account must change its password before doing anything else.
    ForcedChange,
}

#[derive(Debug, thiserror::Error)]
pub enum BootstrapError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("failed to hash admin password: {0}")]
    Hash(String),
}

/// Bring the seeded admin account to a safe state. Call after migrations.
pub async fn ensure_default_admin(
    pool: &SqlitePool,
    config: &Config,
) -> Result<AdminBootstrap, BootstrapError> {
    let Some(admin) = sqlx::query!(
        "SELECT email, password_hash, must_change_password, last_login_at FROM users WHERE id = ?",
        SEED_ADMIN_ID
    )
    .fetch_optional(pool)
    .await?
    else {
        return Ok(AdminBootstrap::NoSeedAccount);
    };

    let on_seed_password = admin.password_hash == SEED_ADMIN_HASH;
    // A generated password nobody has signed in with is still ours to replace,
    // which lets an operator who lost the log line recover by restarting.
    let generated_and_unused =
        !on_seed_password && admin.must_change_password != 0 && admin.last_login_at.is_none();

    if !on_seed_password && !generated_and_unused {
        return Ok(AdminBootstrap::AlreadyConfigured);
    }

    if let Some(password) = usable_configured_password(config) {
        let email = apply_configured_email(pool, &admin.email, config).await?;
        set_password(pool, password, config, false).await?;
        tracing::info!("Admin account {} configured from the environment", email);
        return Ok(AdminBootstrap::AppliedConfigured);
    }

    if is_fresh_install(pool).await? {
        let email = apply_configured_email(pool, &admin.email, config).await?;
        let password = generate_password();
        set_password(pool, &password, config, true).await?;
        tracing::warn!(
            "\n\n  Initial admin account\n    Email:    {}\n    Password: {}\n\n  \
             This password is shown only once and must be changed at first sign-in.\n  \
             Set DEFAULT_ADMIN_PASSWORD to choose your own. If you lose it before signing in,\n  \
             restart the server to generate another.\n",
            email,
            password
        );
        return Ok(AdminBootstrap::GeneratedPassword(password));
    }

    if !on_seed_password {
        return Ok(AdminBootstrap::AlreadyConfigured);
    }

    sqlx::query!(
        "UPDATE users SET must_change_password = 1 WHERE id = ?",
        SEED_ADMIN_ID
    )
    .execute(pool)
    .await?;
    tracing::warn!(
        "SECURITY: admin account {} still has the published default password. Admin routes \
         stay blocked until it is changed: sign in at /admin and set a new one, or set \
         DEFAULT_ADMIN_PASSWORD and restart.",
        admin.email
    );
    Ok(AdminBootstrap::ForcedChange)
}

/// The configured password, unless it is missing or fails the password policy.
/// The policy's minimum length rejects the published default, which older
/// `.env` files set explicitly.
fn usable_configured_password(config: &Config) -> Option<&str> {
    let password = config.default_admin_password.as_deref()?;
    match validate_password_strength(password, &config.password_complexity) {
        Ok(()) => Some(password),
        Err(reason) => {
            tracing::warn!("Ignoring DEFAULT_ADMIN_PASSWORD: {}", reason);
            None
        }
    }
}

/// An install is fresh when nothing has ever used it. Registration is open, so
/// a pending account doesn't count: nobody can approve it yet.
async fn is_fresh_install(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT
            (SELECT COUNT(*) FROM users WHERE approved = 1 AND id != ?1) AS "other_users!: i64",
            (SELECT COUNT(*) FROM clients) AS "devices!: i64",
            (SELECT COUNT(*) FROM notes) AS "notes!: i64",
            (SELECT last_login_at FROM users WHERE id = ?1) AS "last_login: String""#,
        SEED_ADMIN_ID
    )
    .fetch_one(pool)
    .await?;

    Ok(row.other_users == 0 && row.devices == 0 && row.notes == 0 && row.last_login.is_none())
}

/// Rename the account to `DEFAULT_ADMIN_EMAIL`, unless another account has that
/// address. Returns the address the account ends up with.
async fn apply_configured_email(
    pool: &SqlitePool,
    current: &str,
    config: &Config,
) -> Result<String, sqlx::Error> {
    let wanted = config.default_admin_email.trim();
    if wanted.is_empty() || wanted == current {
        return Ok(current.to_string());
    }

    let taken = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM users WHERE email = ? COLLATE NOCASE AND id != ?",
        wanted,
        SEED_ADMIN_ID
    )
    .fetch_one(pool)
    .await?;
    if taken > 0 {
        tracing::warn!(
            "Ignoring DEFAULT_ADMIN_EMAIL: {} already belongs to another account",
            wanted
        );
        return Ok(current.to_string());
    }

    sqlx::query!("UPDATE users SET email = ? WHERE id = ?", wanted, SEED_ADMIN_ID)
        .execute(pool)
        .await?;
    Ok(wanted.to_string())
}

async fn set_password(
    pool: &SqlitePool,
    password: &str,
    config: &Config,
    must_change: bool,
) -> Result<(), BootstrapError> {
    let hash = hash_password_with_params(
        password,
        config.argon2_m_cost,
        config.argon2_t_cost,
        config.argon2_p_cost,
    )
    .map_err(|e| BootstrapError::Hash(e.to_string()))?;

    let must_change = i64::from(must_change);
    sqlx::query!(
        "UPDATE users SET password_hash = ?, must_change_password = ? WHERE id = ?",
        hash,
        must_change,
        SEED_ADMIN_ID
    )
    .execute(pool)
    .await?;
    Ok(())
}

fn generate_password() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(GENERATED_PASSWORD_LENGTH)
        .map(char::from)
        .collect()
}
