use axum::{extract::State, http::StatusCode, Json};
use rand::Rng;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    db::{SessionRepository, UserRepository},
    error::AppResult,
    models::{
        LoginRequest, LoginResponse, RegisterDeviceRequest, RegisterDeviceResponse,
        RegisterUserRequest, RegisterUserResponse, UserInfo,
    },
    utils::password::{hash_password_with_params, validate_password_strength, verify_password},
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
    let mut hasher = Sha256::new();
    hasher.update(api_key.as_bytes());
    let hashed_key = format!("{:x}", hasher.finalize());

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

/// Admin login endpoint
/// Creates a session for admin dashboard access
pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> AppResult<(StatusCode, Json<LoginResponse>)> {
    tracing::info!("Admin login request: email={}", req.email);

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

    // Create session
    let session = SessionRepository::create(
        &state.pool,
        crate::models::CreateSessionParams {
            user_id: user.id.clone(),
            token: session_token.clone(),
            expires_at: expires_at_str.clone(),
            user_agent: None, // TODO: Extract from request headers
            ip_address: None, // TODO: Extract from request
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

fn is_valid_email(email: &str) -> bool {
    // Basic email validation: local@domain.tld
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return false;
    }

    let local = parts[0];
    let domain = parts[1];

    // Local part must not be empty
    if local.is_empty() {
        return false;
    }

    // Domain must contain a dot and not be empty
    if domain.is_empty() || !domain.contains('.') {
        return false;
    }

    // Domain must have something before and after the dot
    let domain_parts: Vec<&str> = domain.split('.').collect();
    if domain_parts.iter().any(|p| p.is_empty()) {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_validation() {
        assert!(is_valid_email("user@example.com"));
        assert!(is_valid_email("test.user@domain.co.uk"));
        assert!(!is_valid_email("invalid"));
        assert!(!is_valid_email("@example.com"));
        assert!(!is_valid_email("user@"));
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
