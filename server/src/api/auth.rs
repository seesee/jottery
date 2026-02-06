use axum::{extract::State, http::StatusCode, Json};
use rand::Rng;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    db::{SessionRepository, UserRepository},
    error::AppResult,
    models::{
        CloneDeviceRequest, CloneDeviceResponse, LoginRequest, LoginResponse,
        RegisterDeviceRequest, RegisterDeviceResponse, RegisterUserRequest,
        RegisterUserResponse, UserInfo,
    },
    utils::{
        crypto::hash_sha256,
        password::{hash_password_with_params, validate_password_strength, verify_password},
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
    tracing::info!("User registration request: email={}", req.email);

    // Validate email format
    if !is_valid_email(&req.email) {
        tracing::warn!("Invalid email format: {}", req.email);
        return Err(crate::error::AppError::BadRequest(
            "Invalid email format".to_string(),
        ));
    }

    // Validate password strength based on configured complexity
    if let Err(msg) = validate_password_strength(&req.password, &state.config.password_complexity) {
        tracing::warn!("Password validation failed for {}: {}", req.email, msg);
        return Err(crate::error::AppError::BadRequest(msg));
    }

    // Check if email already exists
    if UserRepository::email_exists(&state.pool, &req.email).await? {
        tracing::warn!("Email already registered: {}", req.email);
        return Err(crate::error::AppError::Conflict(
            "Email already registered".to_string(),
        ));
    }

    // Hash password with configured Argon2 parameters
    let password_hash = hash_password_with_params(
        &req.password,
        state.config.argon2_m_cost,
        state.config.argon2_t_cost,
        state.config.argon2_p_cost,
    )
    .map_err(|e| {
        tracing::error!("Password hashing failed: {}", e);
        crate::error::AppError::InternalServerError
    })?;

    // Create user (approved=false by default)
    let user = UserRepository::create(&state.pool, &req.email, &password_hash).await?;

    tracing::info!(
        "User registered successfully: id={}, email={}, pending approval",
        user.id,
        user.email
    );

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
    tracing::info!(
        "Device registration request: email={}, device={}",
        req.email,
        req.device_name
    );

    // Validate device name
    validation::validate_device_name(&req.device_name, &state.config)?;

    // Get user by email
    let user = UserRepository::get_by_email(&state.pool, &req.email)
        .await
        .map_err(|_| {
            tracing::warn!("Device registration failed: user not found: {}", req.email);
            crate::error::AppError::Unauthorized
        })?;

    // Verify password
    let password_valid = verify_password(&req.password, &user.password_hash).map_err(|e| {
        tracing::error!("Password verification failed: {}", e);
        crate::error::AppError::InternalServerError
    })?;

    if !password_valid {
        tracing::warn!("Device registration failed: invalid password for {}", req.email);
        return Err(crate::error::AppError::Unauthorized);
    }

    // Check if user is approved
    if user.approved == 0 {
        tracing::warn!(
            "Device registration failed: user not approved: {}",
            req.email
        );
        return Err(crate::error::AppError::Forbidden(
            "Account pending admin approval".to_string(),
        ));
    }

    // Check if user is active
    if user.is_active == 0 {
        tracing::warn!("Device registration failed: user deactivated: {}", req.email);
        return Err(crate::error::AppError::Forbidden(
            "Account deactivated".to_string(),
        ));
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
        "Device registered successfully: client_id={}, user_id={}, device={}",
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
    tracing::info!(
        "Clone device request: device_name={}, device_type={}",
        req.device_name,
        req.device_type
    );

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
        tracing::warn!(
            "Clone device failed: user not approved: {}",
            user.email
        );
        return Err(crate::error::AppError::Forbidden(
            "Account pending admin approval".to_string(),
        ));
    }

    // Check if user is active
    if user.is_active != 1 {
        tracing::warn!("Clone device failed: user deactivated: {}", user.email);
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
        "Device cloned successfully: new_client_id={}, user_id={}, device={}, from_client={}",
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
    headers: axum::http::HeaderMap,
    Json(req): Json<LoginRequest>,
) -> AppResult<(StatusCode, Json<LoginResponse>)> {
    tracing::info!("Admin login request: email={}", req.email);

    // Extract client info for audit trail
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Extract IP from X-Forwarded-For (first IP in chain) or X-Real-IP
    let ip_address = headers
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("X-Real-IP")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        });

    // Get user by email
    let user = UserRepository::get_by_email(&state.pool, &req.email)
        .await
        .map_err(|_| {
            tracing::warn!("Login failed: user not found: {}", req.email);
            crate::error::AppError::Unauthorized
        })?;

    // Verify password
    let password_valid = verify_password(&req.password, &user.password_hash).map_err(|e| {
        tracing::error!("Password verification failed: {}", e);
        crate::error::AppError::InternalServerError
    })?;

    if !password_valid {
        tracing::warn!("Login failed: invalid password for {}", req.email);
        return Err(crate::error::AppError::Unauthorized);
    }

    // Check if user is admin
    if user.is_admin == 0 {
        tracing::warn!("Login failed: user is not admin: {}", req.email);
        return Err(crate::error::AppError::Forbidden(
            "Admin access required".to_string(),
        ));
    }

    // Check if user is active
    if user.is_active == 0 {
        tracing::warn!("Login failed: user deactivated: {}", req.email);
        return Err(crate::error::AppError::Forbidden(
            "Account deactivated".to_string(),
        ));
    }

    // Generate session token
    let session_token = generate_session_token();

    // Calculate expiry (configurable, default 7 days)
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

    tracing::info!(
        "Login successful: user_id={}, session_id={}, email={}",
        user.id,
        session.id,
        user.email
    );

    let response = LoginResponse {
        session_id: session_token, // Return the token, not the session.id
        expires_at: expires_at_str,
        user: UserInfo {
            id: user.id,
            email: user.email,
            is_admin: user.is_admin == 1,
        },
    };

    Ok((StatusCode::OK, Json(response)))
}

/// Logout endpoint
/// Invalidates the current session
#[allow(dead_code)]
pub async fn logout(
    State(state): State<Arc<AppState>>,
    session_id: String, // TODO: Extract from auth middleware
) -> AppResult<StatusCode> {
    tracing::info!("Logout request: session_id={}", session_id);

    SessionRepository::delete(&state.pool, &session_id).await?;

    tracing::info!("Logout successful: session_id={}", session_id);

    Ok(StatusCode::NO_CONTENT)
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
