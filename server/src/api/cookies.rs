//! HttpOnly session cookies for the dashboards.
//!
//! The dashboards share an origin with the notes app, so a token that script can
//! read is a token any script in the notes app can read. A dashboard that asks for
//! it gets a cookie instead. Every other client keeps using a bearer token.
//!
//! A cookie is sent by the browser on its own, which a token never was, so this
//! has to keep cross-site requests out. `SameSite=Strict` stops the cookie being
//! attached to them, and a state-changing request that authenticates with the
//! cookie must also carry a custom header. A cross-site form can't set one, and a
//! cross-origin script can't send one with credentials, because CORS never allows
//! credentials here.

use axum::http::{header, HeaderMap, HeaderValue, Method};

use crate::config::Config;

const COOKIE_NAME: &str = "session_token";

/// Header and value the dashboards send with a cookie session.
pub const CSRF_HEADER: &str = "x-requested-with";
const CSRF_HEADER_VALUE: &str = "JotteryDashboard";

/// Which API a session cookie is sent to. Each dashboard's cookie reaches only its
/// own API, so a portal session isn't sent to the admin routes.
#[derive(Clone, Copy, Debug)]
pub enum CookieScope {
    Admin,
    User,
}

impl CookieScope {
    fn path(self) -> &'static str {
        match self {
            CookieScope::Admin => "/api/v1/admin",
            CookieScope::User => "/api/v1/user",
        }
    }
}

fn cookie_header(value: &str, max_age_secs: i64, scope: CookieScope) -> HeaderMap {
    let cookie = format!(
        "{COOKIE_NAME}={value}; HttpOnly; Secure; SameSite=Strict; Max-Age={max_age_secs}; Path={}",
        scope.path()
    );
    let mut headers = HeaderMap::new();
    // Tokens are generated here as hex or a UUID, so this can't fail on real input
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        headers.insert(header::SET_COOKIE, value);
    }
    headers
}

/// How a new session reaches the client.
pub struct SessionGrant {
    /// The token, unless it went into a cookie.
    pub session_id: Option<String>,
    pub cookie_session: bool,
    pub headers: HeaderMap,
}

/// Decide how to hand over a new session.
///
/// `Secure` cookies never come back over plain HTTP, and `X-Forwarded-Proto` can't
/// be trusted here, so a client only gets a cookie on an install that has said it
/// serves HTTPS, by setting ENABLE_HSTS. Anything else gets the token as before.
pub fn grant_session(
    config: &Config,
    scope: CookieScope,
    requested_cookie: bool,
    token: String,
) -> SessionGrant {
    if requested_cookie && config.enable_hsts {
        SessionGrant {
            headers: cookie_header(&token, config.session_expiry_days * 24 * 60 * 60, scope),
            session_id: None,
            cookie_session: true,
        }
    } else {
        SessionGrant {
            session_id: Some(token),
            cookie_session: false,
            headers: HeaderMap::new(),
        }
    }
}

/// Headers that remove a session cookie from the browser.
pub fn clear_session_cookie(scope: CookieScope) -> HeaderMap {
    cookie_header("", 0, scope)
}

/// The session token a request carries, and how it arrived.
#[derive(Debug, PartialEq, Eq)]
pub struct SessionCredential {
    pub token: String,
    pub from_cookie: bool,
}

/// Find the session token in a request. Authorization wins over the cookie.
pub fn session_credential(headers: &HeaderMap) -> Option<SessionCredential> {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if let Some(token) = bearer {
        return Some(SessionCredential {
            token: token.to_string(),
            from_cookie: false,
        });
    }

    let cookies = headers.get(header::COOKIE)?.to_str().ok()?;
    cookies.split(';').find_map(|cookie| {
        let (name, value) = cookie.trim().split_once('=')?;
        (name == COOKIE_NAME).then(|| SessionCredential {
            token: value.to_string(),
            from_cookie: true,
        })
    })
}

/// Whether a request that authenticated with `credential` could have been forged
/// by another site. A request that can't change anything has nothing to forge, and
/// every other method is treated as one that can.
pub fn is_forgeable(credential: &SessionCredential, method: &Method, headers: &HeaderMap) -> bool {
    credential.from_cookie
        && !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
        && headers.get(CSRF_HEADER).map(HeaderValue::as_bytes) != Some(CSRF_HEADER_VALUE.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bearer_token_is_found() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, HeaderValue::from_static("Bearer test-token-123"));

        assert_eq!(
            session_credential(&headers),
            Some(SessionCredential { token: "test-token-123".to_string(), from_cookie: false })
        );
    }

    #[test]
    fn a_cookie_is_found_among_others() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("other=value; session_token=test-token-456; another=x"),
        );

        assert_eq!(
            session_credential(&headers),
            Some(SessionCredential { token: "test-token-456".to_string(), from_cookie: true })
        );
    }

    #[test]
    fn a_bearer_token_wins_over_a_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, HeaderValue::from_static("Bearer from-header"));
        headers.insert(header::COOKIE, HeaderValue::from_static("session_token=from-cookie"));

        let credential = session_credential(&headers).unwrap();

        assert_eq!(credential.token, "from-header");
        assert!(!credential.from_cookie);
    }

    #[test]
    fn no_credential_is_none() {
        assert_eq!(session_credential(&HeaderMap::new()), None);
    }

    #[test]
    fn methods_nothing_routes_are_still_treated_as_writes() {
        let credential = SessionCredential { token: "t".to_string(), from_cookie: true };

        assert!(is_forgeable(&credential, &Method::TRACE, &HeaderMap::new()));
        assert!(!is_forgeable(&credential, &Method::GET, &HeaderMap::new()));
    }
}
