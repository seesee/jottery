//! Custom Axum extractors for authentication
//!
//! These extractors retrieve authentication info from request extensions,
//! which are populated by the authentication middleware.

use crate::{
    api::middleware::{ClientInfo, InboxAuth},
    error::AppError,
};

/// Extractor for device API key authentication
///
/// Retrieves ClientInfo (client_id + user_id) from request extensions.
/// Used for sync and inbox management endpoints that require device authentication.
pub struct AuthClient(pub ClientInfo);

#[axum::async_trait]
impl<S> axum::extract::FromRequestParts<S> for AuthClient
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<ClientInfo>()
            .cloned()
            .map(AuthClient)
            .ok_or(AppError::Unauthorized)
    }
}

/// Extractor for inbox token authentication (limited scope)
///
/// Retrieves InboxAuth (user_id only) from request extensions.
/// Used for inbox submission endpoint which uses a separate, limited-scope token.
pub struct InboxAuthUser(pub InboxAuth);

#[axum::async_trait]
impl<S> axum::extract::FromRequestParts<S> for InboxAuthUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<InboxAuth>()
            .cloned()
            .map(InboxAuthUser)
            .ok_or(AppError::Unauthorized)
    }
}
