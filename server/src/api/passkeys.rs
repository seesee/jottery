//! WebAuthn / passkey endpoints for the user portal.
//!
//! Flow overview
//! =============
//!
//! **Enrolment** (user is fully authenticated with password; mfa_verified=1):
//!
//!   1. Client → `POST /passkeys/register/begin` with optional `nickname`
//!   2. Server builds a `PasskeyRegistration` challenge, stores it in
//!      `webauthn_challenges` keyed by a challenge_id, returns
//!      `{ challengeId, publicKey }` where `publicKey` is the CCR JSON the
//!      browser's `navigator.credentials.create()` expects.
//!   3. Browser invokes WebAuthn; user approves with biometric / PIN /
//!      security key; returns a credential response.
//!   4. Client → `POST /passkeys/register/complete` with `{ challengeId,
//!      nickname, response }`.
//!   5. Server loads the stored `PasskeyRegistration`, verifies the
//!      response via `finish_passkey_registration`, persists the
//!      resulting `Passkey` to `user_credentials`, deletes the challenge.
//!
//! **Login** (user has just presented password; session.mfa_verified=0):
//!
//!   1. Client → `POST /passkeys/authenticate/begin` (auth: mfa-pending
//!      session)
//!   2. Server loads all of the user's passkeys, builds a
//!      `PasskeyAuthentication` challenge, stores it, returns
//!      `{ challengeId, publicKey }`.
//!   3. Browser invokes `navigator.credentials.get()`, returns assertion.
//!   4. Client → `POST /passkeys/authenticate/complete`.
//!   5. Server verifies assertion, updates the matching credential's
//!      sign_count, flips `sessions.mfa_verified` to 1.
//!
//! **Management** (mfa_verified=1 sessions only):
//!
//!   - `GET  /passkeys`         → list a user's enrolled passkeys
//!   - `DELETE /passkeys/:id`   → remove a passkey (irreversible)
//!
//! All challenge state is stored server-side so a stolen challenge_id on
//! its own grants no access; the browser must produce a valid signed
//! assertion bound to the challenge bytes.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use webauthn_rs::prelude::*;

use crate::{
    db::SessionRepository,
    error::{AppError, AppResult},
    models::Session,
    AppState,
};

// --- DTOs --------------------------------------------------------------

/// Registration is initiated without any body — the nickname is captured
/// at `complete_registration` time so the client can show its label UI
/// after the browser prompt returns. The empty struct is kept so the
/// route signature remains `Json<T>`.
#[derive(Debug, Deserialize)]
pub struct BeginRegistrationRequest {}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BeginRegistrationResponse {
    pub challenge_id: String,
    pub public_key: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteRegistrationRequest {
    pub challenge_id: String,
    pub nickname: Option<String>,
    /// RegisterPublicKeyCredential — the JSON the browser returns from
    /// `navigator.credentials.create()`, passed through untouched.
    pub response: serde_json::Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PasskeyListItem {
    pub id: String,
    pub nickname: Option<String>,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BeginAuthenticationResponse {
    pub challenge_id: String,
    pub public_key: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteAuthenticationRequest {
    pub challenge_id: String,
    /// PublicKeyCredential returned from `navigator.credentials.get()`.
    pub response: serde_json::Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteAuthenticationResponse {
    /// True once the session.mfa_verified flag has been flipped. Client
    /// should now treat its session token as fully usable.
    pub authenticated: bool,
}

// --- Helpers -----------------------------------------------------------

const CHALLENGE_TTL_SECONDS: i64 = 300;

/// Fail the request with 404 if passkey support isn't configured. Done
/// per-handler rather than via middleware so the user portal can hide the
/// passkey UI entirely when the server doesn't offer it.
fn require_webauthn(state: &AppState) -> AppResult<Arc<Webauthn>> {
    state
        .webauthn
        .clone()
        .ok_or_else(|| AppError::NotFound("passkeys are not enabled on this server".to_string()))
}

async fn prune_expired_challenges(pool: &sqlx::SqlitePool) {
    let now = chrono::Utc::now().to_rfc3339();
    let _ = sqlx::query!("DELETE FROM webauthn_challenges WHERE expires_at < ?", now)
        .execute(pool)
        .await;
}

/// Pull a stored webauthn-rs state JSON blob out of the challenges table,
/// verifying that it matches the claimed user + kind. Always deletes the
/// row (one-shot semantics) so a replayed challenge_id can't be reused.
async fn consume_challenge(
    pool: &sqlx::SqlitePool,
    challenge_id: &str,
    user_id: &str,
    kind: &str,
) -> AppResult<String> {
    let row = sqlx::query!(
        r#"SELECT state_json, expires_at, user_id, kind FROM webauthn_challenges WHERE id = ?"#,
        challenge_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("challenge not found or already used".to_string()))?;

    let _ = sqlx::query!("DELETE FROM webauthn_challenges WHERE id = ?", challenge_id)
        .execute(pool)
        .await;

    if row.user_id != user_id {
        return Err(AppError::Unauthorized);
    }
    if row.kind != kind {
        return Err(AppError::BadRequest("challenge kind mismatch".to_string()));
    }
    let now = chrono::Utc::now().to_rfc3339();
    if row.expires_at < now {
        return Err(AppError::BadRequest("challenge expired".to_string()));
    }
    Ok(row.state_json)
}

// --- Registration ------------------------------------------------------

pub async fn begin_registration(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
    Json(_req): Json<BeginRegistrationRequest>,
) -> AppResult<Json<BeginRegistrationResponse>> {
    let webauthn = require_webauthn(&state)?;
    prune_expired_challenges(&state.pool).await;

    // Fetch email + any already-enrolled credentials so we can pass
    // `exclude_credentials` and prevent registering the same authenticator
    // twice against the same account.
    let user = sqlx::query!(
        "SELECT id, email FROM users WHERE id = ?",
        session.user_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    let existing = sqlx::query!(
        "SELECT passkey_json FROM user_credentials WHERE user_id = ?",
        session.user_id
    )
    .fetch_all(&state.pool)
    .await?;

    let exclude: Vec<CredentialID> = existing
        .into_iter()
        .filter_map(|r| serde_json::from_str::<Passkey>(&r.passkey_json).ok())
        .map(|p| p.cred_id().clone())
        .collect();

    // WebAuthn spec wants a UUID for the user handle. We re-use our
    // user_id (which is already a UUID string) so it's stable across
    // credential enrolments on the same account.
    let user_uuid = Uuid::parse_str(&user.id).map_err(|_| AppError::InternalServerError)?;

    let (ccr, reg_state) = webauthn
        .start_passkey_registration(user_uuid, &user.email, &user.email, Some(exclude))
        .map_err(|e| {
            tracing::error!("start_passkey_registration failed: {:?}", e);
            AppError::InternalServerError
        })?;

    let challenge_id = Uuid::new_v4().to_string();
    let state_json = serde_json::to_string(&reg_state).map_err(|_| AppError::InternalServerError)?;
    let now = chrono::Utc::now().to_rfc3339();
    let expires_at = (chrono::Utc::now() + chrono::Duration::seconds(CHALLENGE_TTL_SECONDS)).to_rfc3339();

    sqlx::query!(
        r#"INSERT INTO webauthn_challenges (id, user_id, kind, state_json, created_at, expires_at)
           VALUES (?, ?, 'registration', ?, ?, ?)"#,
        challenge_id,
        session.user_id,
        state_json,
        now,
        expires_at,
    )
    .execute(&state.pool)
    .await?;

    let public_key = serde_json::to_value(&ccr).map_err(|_| AppError::InternalServerError)?;

    Ok(Json(BeginRegistrationResponse { challenge_id, public_key }))
}

pub async fn complete_registration(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
    Json(req): Json<CompleteRegistrationRequest>,
) -> AppResult<(StatusCode, Json<PasskeyListItem>)> {
    let webauthn = require_webauthn(&state)?;

    let state_json = consume_challenge(&state.pool, &req.challenge_id, &session.user_id, "registration").await?;
    let reg_state: PasskeyRegistration =
        serde_json::from_str(&state_json).map_err(|_| AppError::InternalServerError)?;

    let register_response: RegisterPublicKeyCredential =
        serde_json::from_value(req.response).map_err(|e| {
            AppError::BadRequest(format!("invalid credential response: {}", e))
        })?;

    let passkey = webauthn
        .finish_passkey_registration(&register_response, &reg_state)
        .map_err(|e| {
            tracing::warn!("finish_passkey_registration rejected: {:?}", e);
            AppError::BadRequest("passkey registration failed verification".to_string())
        })?;

    let cred_id = base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, passkey.cred_id().as_ref());
    let passkey_json = serde_json::to_string(&passkey).map_err(|_| AppError::InternalServerError)?;
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query!(
        r#"INSERT INTO user_credentials (id, user_id, passkey_json, nickname, created_at)
           VALUES (?, ?, ?, ?, ?)"#,
        cred_id,
        session.user_id,
        passkey_json,
        req.nickname,
        now,
    )
    .execute(&state.pool)
    .await?;

    tracing::info!("Passkey registered for user {} ({})", session.user_id, cred_id);

    Ok((
        StatusCode::CREATED,
        Json(PasskeyListItem {
            id: cred_id,
            nickname: req.nickname,
            created_at: now,
            last_used_at: None,
        }),
    ))
}

// --- Authentication ----------------------------------------------------

pub async fn begin_authentication(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
) -> AppResult<Json<BeginAuthenticationResponse>> {
    let webauthn = require_webauthn(&state)?;
    prune_expired_challenges(&state.pool).await;

    let rows = sqlx::query!(
        "SELECT passkey_json FROM user_credentials WHERE user_id = ?",
        session.user_id
    )
    .fetch_all(&state.pool)
    .await?;

    if rows.is_empty() {
        // This shouldn't happen in normal flow — login only stages
        // mfa_pending when the user has credentials. Defensive 400 in
        // case the only credential was deleted between login and this
        // call, or if an attacker forged the session state.
        return Err(AppError::BadRequest("no passkeys enrolled".to_string()));
    }

    let passkeys: Vec<Passkey> = rows
        .into_iter()
        .filter_map(|r| serde_json::from_str::<Passkey>(&r.passkey_json).ok())
        .collect();

    let (rcr, auth_state) = webauthn.start_passkey_authentication(&passkeys).map_err(|e| {
        tracing::error!("start_passkey_authentication failed: {:?}", e);
        AppError::InternalServerError
    })?;

    let challenge_id = Uuid::new_v4().to_string();
    let state_json = serde_json::to_string(&auth_state).map_err(|_| AppError::InternalServerError)?;
    let now = chrono::Utc::now().to_rfc3339();
    let expires_at = (chrono::Utc::now() + chrono::Duration::seconds(CHALLENGE_TTL_SECONDS)).to_rfc3339();

    sqlx::query!(
        r#"INSERT INTO webauthn_challenges (id, user_id, kind, state_json, created_at, expires_at)
           VALUES (?, ?, 'authentication', ?, ?, ?)"#,
        challenge_id,
        session.user_id,
        state_json,
        now,
        expires_at,
    )
    .execute(&state.pool)
    .await?;

    let public_key = serde_json::to_value(&rcr).map_err(|_| AppError::InternalServerError)?;

    Ok(Json(BeginAuthenticationResponse { challenge_id, public_key }))
}

pub async fn complete_authentication(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
    Json(req): Json<CompleteAuthenticationRequest>,
) -> AppResult<Json<CompleteAuthenticationResponse>> {
    let webauthn = require_webauthn(&state)?;

    let state_json = consume_challenge(&state.pool, &req.challenge_id, &session.user_id, "authentication").await?;
    let auth_state: PasskeyAuthentication =
        serde_json::from_str(&state_json).map_err(|_| AppError::InternalServerError)?;

    let auth_response: PublicKeyCredential = serde_json::from_value(req.response)
        .map_err(|e| AppError::BadRequest(format!("invalid credential response: {}", e)))?;

    let auth_result = webauthn
        .finish_passkey_authentication(&auth_response, &auth_state)
        .map_err(|e| {
            tracing::warn!("finish_passkey_authentication rejected: {:?}", e);
            AppError::Unauthorized
        })?;

    // If the authenticator reported a new counter, persist it by rewriting
    // the stored Passkey. Counter rollback is already handled by
    // finish_passkey_authentication (which would have errored); the job
    // here is just to commit the advance.
    let cred_id_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        auth_result.cred_id().as_ref(),
    );

    if auth_result.needs_update() {
        let existing = sqlx::query!(
            "SELECT passkey_json FROM user_credentials WHERE id = ? AND user_id = ?",
            cred_id_b64,
            session.user_id,
        )
        .fetch_optional(&state.pool)
        .await?;

        if let Some(row) = existing {
            if let Ok(mut pk) = serde_json::from_str::<Passkey>(&row.passkey_json) {
                let _ = pk.update_credential(&auth_result);
                if let Ok(updated) = serde_json::to_string(&pk) {
                    let now = chrono::Utc::now().to_rfc3339();
                    let _ = sqlx::query!(
                        "UPDATE user_credentials SET passkey_json = ?, last_used_at = ? WHERE id = ?",
                        updated,
                        now,
                        cred_id_b64,
                    )
                    .execute(&state.pool)
                    .await;
                }
            }
        }
    } else {
        // Even if the counter didn't change, record the usage for UX.
        let now = chrono::Utc::now().to_rfc3339();
        let _ = sqlx::query!(
            "UPDATE user_credentials SET last_used_at = ? WHERE id = ?",
            now,
            cred_id_b64,
        )
        .execute(&state.pool)
        .await;
    }

    // Promote the session.
    SessionRepository::mark_mfa_verified(&state.pool, &session.id).await?;
    tracing::info!("Passkey authentication completed for user {}", session.user_id);

    Ok(Json(CompleteAuthenticationResponse { authenticated: true }))
}

// --- Management --------------------------------------------------------

pub async fn list_passkeys(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
) -> AppResult<Json<Vec<PasskeyListItem>>> {
    require_webauthn(&state)?;

    let rows = sqlx::query!(
        r#"SELECT id, nickname, created_at, last_used_at
           FROM user_credentials WHERE user_id = ? ORDER BY created_at DESC"#,
        session.user_id,
    )
    .fetch_all(&state.pool)
    .await?;

    let items = rows
        .into_iter()
        .map(|r| PasskeyListItem {
            id: r.id,
            nickname: r.nickname,
            created_at: r.created_at,
            last_used_at: r.last_used_at,
        })
        .collect();

    Ok(Json(items))
}

pub async fn delete_passkey(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
    Path(cred_id): Path<String>,
) -> AppResult<StatusCode> {
    require_webauthn(&state)?;

    let result = sqlx::query!(
        "DELETE FROM user_credentials WHERE id = ? AND user_id = ?",
        cred_id,
        session.user_id,
    )
    .execute(&state.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("passkey not found".to_string()));
    }

    tracing::info!("Passkey {} deleted by user {}", cred_id, session.user_id);
    Ok(StatusCode::NO_CONTENT)
}
