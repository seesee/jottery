use axum::{
    extract::{ConnectInfo, State},
    http::StatusCode,
    Json,
};
use rand::Rng;
use std::{net::SocketAddr, sync::Arc};
use uuid::Uuid;

use crate::{
    api::cookies::{grant_session, CookieScope},
    db::{SessionRepository, UserRepository},
    error::AppResult,
    models::{
        CloneDeviceRequest, CloneDeviceResponse, LoginRequest, LoginResponse,
        RegisterDeviceRequest, RegisterDeviceResponse, RegisterUserRequest,
        RegisterUserResponse, UserInfo,
    },
    utils::{
        crypto::{hash_sha256, log_id},
        password::{hash_for_storage, validate_password_strength, verify_sign_in},
        validation,
    },
    AppState,
};

/// Register a new user (email + password)
/// User starts in pending approval state
pub async fn register_user(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterUserRequest>,
) -> AppResult<(StatusCode, Json<RegisterUserResponse>)> {
    // Tidied before anything else sees it, so that one mailbox is one account
    let email = req.email.trim().to_ascii_lowercase();

    // Validate email format
    if !is_valid_email(&email) {
        tracing::warn!("Registration refused: invalid email format");
        return Err(crate::error::AppError::BadRequest(
            "Invalid email format".to_string(),
        ));
    }

    // Validate password strength based on configured complexity
    if let Err(msg) = validate_password_strength(&req.password, &state.config.password_complexity) {
        tracing::warn!("Registration refused: {}", msg);
        return Err(crate::error::AppError::BadRequest(msg));
    }

    // Check if email already exists
    let email_taken = || {
        tracing::warn!("Registration refused: address {} is taken", log_id(&email));
        crate::error::AppError::Conflict("Email already registered".to_string())
    };

    if UserRepository::email_exists(&state.pool, &email).await? {
        return Err(email_taken());
    }

    // Hash password with configured Argon2 parameters
    let password_hash = hash_for_storage(&state.config, &req.password)
        .await
        .map_err(|e| {
            tracing::error!("Password hashing failed: {}", e);
            crate::error::AppError::InternalServerError
        })?;

    // Create user (approved=false by default)
    let user = UserRepository::create(&state.pool, &email, &password_hash, &state.config)
        .await?
        .ok_or_else(email_taken)?;

    tracing::info!("User registered: id={}, pending approval", user.id);

    let response = RegisterUserResponse {
        user_id: user.id,
        email: user.email,
        status: "pending_approval".to_string(),
        message: "Account created. Awaiting admin approval.".to_string(),
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// Register a new device (requires user authentication)
/// User must be approved to register devices
pub async fn register_device(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterDeviceRequest>,
) -> AppResult<(StatusCode, Json<RegisterDeviceResponse>)> {
    // Validate device name
    validation::validate_device_name(&req.device_name, &state.config)?;

    state.login_throttle.check(&req.email)?;

    // Get user by email
    let user = UserRepository::find_by_email(&state.pool, &req.email).await?;

    // Verify password
    let password_valid = verify_sign_in(
        &state.config,
        &req.password,
        user.as_ref().map(|user| user.password_hash.as_str()),
    )
    .await
    .map_err(|e| {
        tracing::error!("Password verification failed: {}", e);
        crate::error::AppError::InternalServerError
    })?;

    let Some(user) = user.filter(|_| password_valid) else {
        tracing::warn!("Device registration failed: bad credentials for {}", log_id(&req.email));
        state.login_throttle.record_failure(&req.email);
        return Err(crate::error::AppError::Unauthorized);
    };
    state.login_throttle.record_success(&req.email);

    // Check if user is approved
    if user.approved == 0 {
        tracing::warn!("Device registration failed: user not approved: {}", user.id);
        return Err(crate::error::AppError::Forbidden(
            "Account pending admin approval".to_string(),
        ));
    }

    // Check if user is active
    if user.is_active == 0 {
        tracing::warn!("Device registration failed: user deactivated: {}", user.id);
        return Err(crate::error::AppError::Forbidden(
            "Account deactivated".to_string(),
        ));
    }

    // A password that must be changed can't be used to enrol anything
    if user.must_change_password != 0 {
        tracing::warn!("Device registration refused: {} must change password", user.id);
        return Err(crate::error::AppError::PasswordChangeRequired);
    }

    // Generate client ID
    let client_id = Uuid::new_v4().to_string();

    // Generate random API key
    let api_key = generate_api_key();

    // Hash API key for storage
    let hashed_key = hash_sha256(&api_key);

    // Current timestamp
    let now = chrono::Utc::now().to_rfc3339();

    // Insert device into database
    sqlx::query!(
        r#"
        INSERT INTO clients (id, user_id, api_key, device_name, device_type, created_at, last_seen_at, is_active)
        VALUES (?, ?, ?, ?, ?, ?, ?, 1)
        "#,
        client_id,
        user.id,
        hashed_key,
        req.device_name,
        req.device_type,
        now,
        now
    )
    .execute(&state.pool)
    .await?;

    tracing::info!(
        "Device registered successfully: client_id={}, user_id={}, device={:?}",
        client_id,
        user.id,
        req.device_name
    );

    let response = RegisterDeviceResponse {
        client_id,
        api_key,
        user_id: user.id,
        device_name: req.device_name,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// Clone/register a new device using an existing valid API key
/// Used when importing sync credentials on a new device
/// Creates a new device for the same user with a new client ID and API key
pub async fn clone_device(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CloneDeviceRequest>,
) -> AppResult<(StatusCode, Json<CloneDeviceResponse>)> {
    // Validate device name
    validation::validate_device_name(&req.device_name, &state.config)?;

    // Hash the provided API key to look up the source device
    let hashed_key = hash_sha256(&req.api_key);

    // Find the client by API key
    let source_client = sqlx::query!(
        "SELECT id, user_id, is_active FROM clients WHERE api_key = ?",
        hashed_key
    )
    .fetch_optional(&state.pool)
    .await?;

    let source_client = match source_client {
        Some(c) if c.is_active == Some(1) => c,
        Some(_) => {
            tracing::warn!("Clone device failed: source device is inactive");
            return Err(crate::error::AppError::Unauthorized);
        }
        None => {
            tracing::warn!("Clone device failed: invalid API key");
            return Err(crate::error::AppError::Unauthorized);
        }
    };

    // Get the user and verify they're still active and approved
    let user = sqlx::query!(
        "SELECT id, email, approved, is_active FROM users WHERE id = ?",
        source_client.user_id
    )
    .fetch_optional(&state.pool)
    .await?;

    let user = match user {
        Some(u) => u,
        None => {
            tracing::warn!("Clone device failed: user not found");
            return Err(crate::error::AppError::Unauthorized);
        }
    };

    // Check if user is approved
    if user.approved != 1 {
        tracing::warn!("Clone device failed: user not approved: {}", user.id);
        return Err(crate::error::AppError::Forbidden(
            "Account pending admin approval".to_string(),
        ));
    }

    // Check if user is active
    if user.is_active != 1 {
        tracing::warn!("Clone device failed: user deactivated: {}", user.id);
        return Err(crate::error::AppError::Forbidden(
            "Account deactivated".to_string(),
        ));
    }

    // Generate new client ID and API key for the new device
    let client_id = Uuid::new_v4().to_string();
    let api_key = generate_api_key();

    // Hash new API key for storage
    let new_hashed_key = hash_sha256(&api_key);

    let now = chrono::Utc::now().to_rfc3339();

    // Insert the new device
    sqlx::query!(
        r#"
        INSERT INTO clients (id, user_id, api_key, device_name, device_type, created_at, last_seen_at, is_active)
        VALUES (?, ?, ?, ?, ?, ?, ?, 1)
        "#,
        client_id,
        user.id,
        new_hashed_key,
        req.device_name,
        req.device_type,
        now,
        now
    )
    .execute(&state.pool)
    .await?;

    tracing::info!(
        "Device cloned successfully: new_client_id={}, user_id={}, device={:?}, from_client={}",
        client_id,
        user.id,
        req.device_name,
        source_client.id
    );

    let response = CloneDeviceResponse {
        client_id,
        api_key,
        user_id: user.id,
        device_name: req.device_name,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// Admin login endpoint
/// Creates a session for admin dashboard access
pub async fn login(
    State(state): State<Arc<AppState>>,
    peer: Option<ConnectInfo<SocketAddr>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<LoginRequest>,
) -> AppResult<(StatusCode, axum::http::HeaderMap, Json<LoginResponse>)> {
    // Extract client info for audit trail
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Only as far as the trusted proxies allow: anything else is client input
    let ip_address = crate::rate_limit::client_ip(
        &headers,
        peer.map(|ConnectInfo(address)| address.ip()),
        state.config.rate_limit.trusted_proxy_hops,
    )
    .map(|address| address.to_string());

    state.login_throttle.check(&req.email)?;

    // Get user by email
    let user = UserRepository::find_by_email(&state.pool, &req.email).await?;

    // Verify password
    let password_valid = verify_sign_in(
        &state.config,
        &req.password,
        user.as_ref().map(|user| user.password_hash.as_str()),
    )
    .await
    .map_err(|e| {
        tracing::error!("Password verification failed: {}", e);
        crate::error::AppError::InternalServerError
    })?;

    let Some(user) = user.filter(|_| password_valid) else {
        tracing::warn!("Login failed: bad credentials for {}", log_id(&req.email));
        state.login_throttle.record_failure(&req.email);
        return Err(crate::error::AppError::Unauthorized);
    };
    state.login_throttle.record_success(&req.email);

    // Check if user is admin
    if user.is_admin == 0 {
        tracing::warn!("Login failed: user is not admin: {}", user.id);
        return Err(crate::error::AppError::Forbidden(
            "Admin access required".to_string(),
        ));
    }

    // Check if user is approved
    if user.approved == 0 {
        tracing::warn!("Login failed: user not approved: {}", user.id);
        return Err(crate::error::AppError::Forbidden(
            "Account pending admin approval".to_string(),
        ));
    }

    // Check if user is active
    if user.is_active == 0 {
        tracing::warn!("Login failed: user deactivated: {}", user.id);
        return Err(crate::error::AppError::Forbidden(
            "Account deactivated".to_string(),
        ));
    }

    // Generate session token
    let session_token = generate_session_token();

    // Calculate expiry (configurable, default 1 day)
    let expires_at = chrono::Utc::now() + chrono::Duration::days(state.config.session_expiry_days);
    let expires_at_str = expires_at.to_rfc3339();

    // Create session with audit info
    let session = SessionRepository::create(
        &state.pool,
        crate::models::CreateSessionParams {
            user_id: user.id.clone(),
            token: session_token.clone(),
            expires_at: expires_at_str.clone(),
            user_agent,
            ip_address,
        },
    )
    .await?;

    // Update last login timestamp
    UserRepository::update_last_login(&state.pool, &user.id).await?;

    tracing::info!("Login successful: user_id={}, session_id={}", user.id, session.id);

    let grant = grant_session(&state.config, CookieScope::Admin, req.cookie_session, session_token);

    let response = LoginResponse {
        session_id: grant.session_id, // The token, not the session.id
        cookie_session: grant.cookie_session,
        expires_at: expires_at_str,
        user: UserInfo {
            id: user.id,
            email: user.email,
            is_admin: user.is_admin == 1,
        },
        must_change_password: user.must_change_password != 0,
    };

    Ok((StatusCode::OK, grant.headers, Json(response)))
}

// =============================================================================
// Wrapped Key Endpoints (Envelope Encryption)
// =============================================================================

/// Get the wrapped master key for the authenticated user (API key auth)
pub async fn get_wrapped_key(
    State(state): State<Arc<AppState>>,
    axum::Extension(client_info): axum::Extension<crate::api::middleware::ClientInfo>,
) -> AppResult<Json<crate::models::WrappedKeyResponse>> {
    get_wrapped_key_for_user(&state, &client_info.user_id).await
}

/// Get the wrapped master key for the authenticated user (session auth)
pub async fn get_wrapped_key_session(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<crate::models::Session>,
) -> AppResult<Json<crate::models::WrappedKeyResponse>> {
    get_wrapped_key_for_user(&state, &session.user_id).await
}

/// Shared implementation for getting wrapped key
async fn get_wrapped_key_for_user(
    state: &AppState,
    user_id: &str,
) -> AppResult<Json<crate::models::WrappedKeyResponse>> {
    let row = sqlx::query!(
        "SELECT wrapped_blob, kdf_version, kdf_iterations, updated_at FROM wrapped_keys WHERE user_id = ?",
        user_id
    )
    .fetch_optional(&state.pool)
    .await?;

    match row {
        Some(row) => Ok(Json(crate::models::WrappedKeyResponse {
            blob: row.wrapped_blob,
            kdf_version: row.kdf_version as i32,
            kdf_iterations: row.kdf_iterations as i32,
            updated_at: row.updated_at,
        })),
        None => Err(crate::error::AppError::NotFound(
            "No wrapped key found".to_string(),
        )),
    }
}

/// Store or update the wrapped master key for the authenticated user (API key auth)
pub async fn put_wrapped_key(
    State(state): State<Arc<AppState>>,
    axum::Extension(client_info): axum::Extension<crate::api::middleware::ClientInfo>,
    headers: axum::http::HeaderMap,
    Json(req): Json<crate::models::PutWrappedKeyRequest>,
) -> AppResult<StatusCode> {
    put_wrapped_key_for_user(&state, &client_info.user_id, &headers, req).await
}

/// Store or update the wrapped master key for the authenticated user (session auth)
pub async fn put_wrapped_key_session(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<crate::models::Session>,
    headers: axum::http::HeaderMap,
    Json(req): Json<crate::models::PutWrappedKeyRequest>,
) -> AppResult<StatusCode> {
    put_wrapped_key_for_user(&state, &session.user_id, &headers, req).await
}

/// A real blob is a wrapped 32-byte key, a few hundred bytes of JSON. Anything
/// near this size isn't one, and accounts mustn't be free storage.
const MAX_WRAPPED_BLOB_BYTES: usize = 8 * 1024;

/// Reject a wrapped key no client could have produced, or that no client could
/// open: it would stop every new device from onboarding.
pub fn validate_wrapped_key(blob: &str, kdf_version: i32, kdf_iterations: i32) -> AppResult<()> {
    if blob.is_empty() {
        return Err(crate::error::AppError::BadRequest(
            "Wrapped key blob cannot be empty".to_string(),
        ));
    }

    if blob.len() > MAX_WRAPPED_BLOB_BYTES {
        return Err(crate::error::AppError::PayloadTooLarge(format!(
            "Wrapped key blob exceeds maximum size of {} bytes",
            MAX_WRAPPED_BLOB_BYTES
        )));
    }

    if kdf_version < 1 {
        return Err(crate::error::AppError::BadRequest(
            "Invalid KDF version".to_string(),
        ));
    }

    if kdf_iterations < 100_000 {
        return Err(crate::error::AppError::BadRequest(
            "KDF iterations must be at least 100,000".to_string(),
        ));
    }

    Ok(())
}

/// When a client wants its key written.
enum WriteCondition<'a> {
    /// Whatever is stored. A password change asks for this, and it's also what a
    /// released client gets, because those send no condition at all and refusing
    /// them would strand a password change on the old blob.
    Always,
    /// Only over the key last changed at this time, the one the client read.
    IfUnchangedSince(&'a str),
    /// Only if the account has no key yet.
    IfAbsent,
}

fn write_condition<'a>(
    headers: &'a axum::http::HeaderMap,
    replace: Option<bool>,
) -> AppResult<WriteCondition<'a>> {
    if let Some(tag) = headers.get(axum::http::header::IF_MATCH) {
        let tag = tag.to_str().map_err(|_| {
            crate::error::AppError::BadRequest("Unreadable If-Match header".to_string())
        })?;
        return Ok(WriteCondition::IfUnchangedSince(tag.trim().trim_matches('"')));
    }

    Ok(match replace {
        Some(false) => WriteCondition::IfAbsent,
        Some(true) | None => WriteCondition::Always,
    })
}

/// Shared implementation for storing wrapped key
async fn put_wrapped_key_for_user(
    state: &AppState,
    user_id: &str,
    headers: &axum::http::HeaderMap,
    req: crate::models::PutWrappedKeyRequest,
) -> AppResult<StatusCode> {
    validate_wrapped_key(&req.blob, req.kdf_version, req.kdf_iterations)?;

    let now = chrono::Utc::now().to_rfc3339();

    // Each condition is checked by the statement that writes, so two devices
    // acting on the same read can't both succeed.
    let written = match write_condition(headers, req.replace)? {
        WriteCondition::Always => sqlx::query!(
            "INSERT INTO wrapped_keys (user_id, wrapped_blob, kdf_version, kdf_iterations, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(user_id) DO UPDATE SET
               wrapped_blob = excluded.wrapped_blob,
               kdf_version = excluded.kdf_version,
               kdf_iterations = excluded.kdf_iterations,
               updated_at = excluded.updated_at",
            user_id,
            req.blob,
            req.kdf_version,
            req.kdf_iterations,
            now,
            now,
        )
        .execute(&state.pool)
        .await?,
        WriteCondition::IfUnchangedSince(updated_at) => sqlx::query!(
            "UPDATE wrapped_keys
             SET wrapped_blob = ?, kdf_version = ?, kdf_iterations = ?, updated_at = ?
             WHERE user_id = ? AND updated_at = ?",
            req.blob,
            req.kdf_version,
            req.kdf_iterations,
            now,
            user_id,
            updated_at,
        )
        .execute(&state.pool)
        .await?,
        WriteCondition::IfAbsent => sqlx::query!(
            "INSERT INTO wrapped_keys (user_id, wrapped_blob, kdf_version, kdf_iterations, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(user_id) DO NOTHING",
            user_id,
            req.blob,
            req.kdf_version,
            req.kdf_iterations,
            now,
            now,
        )
        .execute(&state.pool)
        .await?,
    };

    if written.rows_affected() == 0 {
        tracing::info!("Wrapped key not stored for user_id={}: it has changed", user_id);
        return Err(crate::error::AppError::Conflict(
            "The wrapped key has changed since it was read".to_string(),
        ));
    }

    tracing::info!("Wrapped key stored for user_id={}", user_id);

    Ok(StatusCode::OK)
}

// Helper functions

fn generate_api_key() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    hex::encode(bytes)
}

fn generate_session_token() -> String {
    Uuid::new_v4().to_string()
}

/// Maximum email length per RFC 5321
const MAX_EMAIL_LENGTH: usize = 254;

fn is_valid_email(email: &str) -> bool {
    // Check length limit per RFC 5321
    if email.len() > MAX_EMAIL_LENGTH {
        return false;
    }

    // Use RFC-compliant email validation
    if !email_address::EmailAddress::is_valid(email) {
        return false;
    }

    // Additional practical check: require at least one dot in the domain
    // This rejects technically valid but practically unusable addresses like "user@localhost"
    if let Some(at_pos) = email.rfind('@') {
        let domain = &email[at_pos + 1..];
        if !domain.contains('.') {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_validation() {
        // Valid emails
        assert!(is_valid_email("user@example.com"));
        assert!(is_valid_email("test.user@domain.co.uk"));
        assert!(is_valid_email("user+tag@example.com"));
        assert!(is_valid_email("user123@sub.domain.example.com"));

        // Invalid emails
        assert!(!is_valid_email("invalid"));
        assert!(!is_valid_email("@example.com"));
        assert!(!is_valid_email("user@"));
        assert!(!is_valid_email("user@.com"));
        assert!(!is_valid_email("user@domain..com"));
        assert!(!is_valid_email(""));
        assert!(!is_valid_email("   "));

        // Require TLD (dot in domain) - reject single-label domains
        assert!(!is_valid_email("user@localhost"));
        assert!(!is_valid_email("user@domain"));

        // RFC length limit (254 chars max)
        let long_local = "a".repeat(64);
        let long_domain = format!("{}.example.com", "b".repeat(180));
        let too_long_email = format!("{}@{}", long_local, long_domain);
        assert!(!is_valid_email(&too_long_email));
    }

    #[test]
    fn test_api_key_generation() {
        let key1 = generate_api_key();
        let key2 = generate_api_key();

        assert_eq!(key1.len(), 64); // 32 bytes = 64 hex chars
        assert_ne!(key1, key2); // Should be unique
    }

    #[test]
    fn test_session_token_generation() {
        let token1 = generate_session_token();
        let token2 = generate_session_token();

        assert_eq!(token1.len(), 36); // UUID format
        assert_ne!(token1, token2); // Should be unique
    }
}
