// API client for Jottery server authentication
// Handles user registration and device registration

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Whether a password and an API key can go to this server without anyone on the way
/// reading them: HTTPS, or plain HTTP to something that isn't reached over the internet.
/// An address that can't be parsed is an error.
pub fn is_safe_endpoint(endpoint: &str) -> Result<bool> {
    let url = reqwest::Url::parse(endpoint).with_context(|| format!("Not a valid address: {endpoint}"))?;

    match url.scheme() {
        "https" => return Ok(true),
        "http" => {}
        _ => anyhow::bail!("Sync endpoint must start with http:// or https://"),
    }

    let host = url.host_str().unwrap_or("").trim_matches(['[', ']']).to_lowercase();

    Ok(match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => is_local_ipv4(ip),
        Ok(std::net::IpAddr::V6(ip)) => match ip.to_ipv4_mapped() {
            Some(ip) => is_local_ipv4(ip),
            None => {
                let first = ip.segments()[0];
                ip.is_loopback() || first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80
            }
        },
        // A single label or a .local name only resolves on the local network
        Err(_) => !host.is_empty() && (!host.contains('.') || host.ends_with(".local")),
    })
}

fn is_local_ipv4(ip: std::net::Ipv4Addr) -> bool {
    ip.is_loopback() || ip.is_private() || ip.is_link_local()
}

/// Refuse an address that would expose a password, unless the user has said they accept
/// that. For commands, which can't stop and ask.
pub fn require_safe_endpoint(endpoint: &str, allow_insecure: bool) -> Result<()> {
    if !is_safe_endpoint(endpoint)? && !allow_insecure {
        anyhow::bail!("{}", rust_i18n::t!("cli.insecure_endpoint", endpoint = endpoint));
    }
    Ok(())
}

/// User registration request
#[derive(Debug, Serialize)]
pub struct RegisterUserRequest {
    pub email: String,
    pub password: String,
}

/// User registration response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterUserResponse {
    pub user_id: String,
    pub email: String,
    pub status: String,
    pub message: String,
}

/// Device registration request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterDeviceRequest {
    pub email: String,
    pub password: String,
    pub device_name: String,
    pub device_type: String,
}

/// Device registration response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterDeviceResponse {
    pub client_id: String,
    pub api_key: String,
    pub user_id: String,
    pub device_name: String,
}

/// Clone device request (for importing credentials)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneDeviceRequest {
    pub api_key: String,
    pub device_name: String,
    pub device_type: String,
}

/// Clone device response (same as RegisterDeviceResponse)
pub type CloneDeviceResponse = RegisterDeviceResponse;

/// User status check response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserStatusResponse {
    pub exists: bool,
    pub is_approved: bool,
    pub is_active: bool,
}

/// API client for authentication endpoints
pub struct AuthClient {
    base_url: String,
    client: reqwest::blocking::Client,
}

impl AuthClient {
    /// Create a new auth client
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::blocking::Client::new(),
        }
    }

    /// Register a new user account
    /// Returns the user ID and approval status
    pub fn register_user(&self, email: &str, password: &str) -> Result<RegisterUserResponse> {
        let url = format!("{}/api/v1/auth/register-user", self.base_url);

        let request = RegisterUserRequest {
            email: email.to_string(),
            password: password.to_string(),
        };

        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .context("Failed to send registration request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Registration failed: {} - {}", status, error_body);
        }

        let result: RegisterUserResponse = response.json()
            .context("Failed to parse registration response")?;

        Ok(result)
    }

    /// Register a device for an existing user
    /// Requires user email and password
    /// Returns API key and client ID for sync
    pub fn register_device(
        &self,
        email: &str,
        password: &str,
        device_name: &str,
        device_type: &str,
    ) -> Result<RegisterDeviceResponse> {
        let url = format!("{}/api/v1/auth/register-device", self.base_url);

        let request = RegisterDeviceRequest {
            email: email.to_string(),
            password: password.to_string(),
            device_name: device_name.to_string(),
            device_type: device_type.to_string(),
        };

        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .context("Failed to send device registration request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().unwrap_or_else(|_| "Unknown error".to_string());

            // Provide user-friendly error messages
            let error_msg = if status == 403 {
                "Your account is pending admin approval or has been deactivated. Please contact the administrator."
            } else if status == 401 {
                "Invalid email or password."
            } else {
                &error_body
            };

            anyhow::bail!("Device registration failed: {}", error_msg);
        }

        let result: RegisterDeviceResponse = response.json()
            .context("Failed to parse device registration response")?;

        Ok(result)
    }

    /// Check if server is reachable
    pub fn health_check(&self) -> Result<bool> {
        let url = format!("{}/health", self.base_url);

        let response = self.client
            .get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .context("Failed to reach server")?;

        Ok(response.status().is_success())
    }

    /// Clone a device using an existing API key
    /// Creates a new device entry for the same user with a new client ID and API key
    /// Used when importing sync credentials on a new device
    pub fn clone_device(
        &self,
        api_key: &str,
        device_name: &str,
        device_type: &str,
    ) -> Result<CloneDeviceResponse> {
        let url = format!("{}/api/v1/auth/clone-device", self.base_url);

        let request = CloneDeviceRequest {
            api_key: api_key.to_string(),
            device_name: device_name.to_string(),
            device_type: device_type.to_string(),
        };

        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .context("Failed to send clone device request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().unwrap_or_else(|_| "Unknown error".to_string());

            // Provide user-friendly error messages
            let error_msg = if status == 403 {
                "Your account is pending admin approval or has been deactivated. Please contact the administrator."
            } else if status == 401 {
                "Invalid API key or device is inactive."
            } else {
                &error_body
            };

            anyhow::bail!("Device cloning failed: {}", error_msg);
        }

        let result: CloneDeviceResponse = response.json()
            .context("Failed to parse clone device response")?;

        Ok(result)
    }

    /// Get the server-escrowed wrapped master key (API key auth)
    /// Returns None if no wrapped key exists (user hasn't migrated to envelope encryption)
    pub fn get_wrapped_key(&self, api_key: &str) -> Result<Option<WrappedKeyResponse>> {
        let url = format!("{}/api/v1/auth/wrapped-key", self.base_url);

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .context("Failed to fetch wrapped key")?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to get wrapped key: {} - {}", status, error_body);
        }

        let result: WrappedKeyResponse = response.json()
            .context("Failed to parse wrapped key response")?;

        Ok(Some(result))
    }

    /// Store or update the server-escrowed wrapped master key (API key auth).
    ///
    /// There's one key per account and the wrong one locks out every new device, so
    /// every write says what it's for. A write the server refuses for that reason comes
    /// back as `KeyWriteResult::Lost`: read the key again and decide again.
    pub fn put_wrapped_key(&self, api_key: &str, request: &PutWrappedKeyRequest, purpose: KeyWrite<'_>) -> Result<KeyWriteResult> {
        let url = format!("{}/api/v1/auth/wrapped-key", self.base_url);

        // `If-Match` only goes to a server that offered an updatedAt: a browser's
        // preflight to an older one wouldn't allow it. Without one, `replace: false`
        // is the safe claim, as older servers ignore it and newer ones always offer one.
        let (replace, if_match) = match purpose {
            KeyWrite::First => (Some(false), None),
            KeyWrite::Repair { updated_at: Some(updated_at) } => (None, Some(updated_at)),
            KeyWrite::Repair { updated_at: None } => (Some(false), None),
            KeyWrite::PasswordChange => (Some(true), None),
        };

        let mut body = serde_json::to_value(request)?;
        if let Some(replace) = replace {
            body["replace"] = serde_json::Value::Bool(replace);
        }

        let mut put = self.client
            .put(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body);
        if let Some(if_match) = if_match {
            put = put.header(reqwest::header::IF_MATCH, if_match);
        }

        let response = put.send().context("Failed to store wrapped key")?;

        if response.status() == reqwest::StatusCode::CONFLICT {
            return Ok(KeyWriteResult::Lost);
        }

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to store wrapped key: {} - {}", status, error_body);
        }

        Ok(KeyWriteResult::Stored)
    }

    /// Check an account's status with its password. A server answers this only for the
    /// right password, unlike the plain check, which tells anyone whether an address
    /// has an account. An older server doesn't have it, so that's the fallback.
    pub fn check_own_status(&self, email: &str, password: &str) -> Result<UserStatusResponse> {
        let url = format!("{}/api/v1/user/status", self.base_url);

        let response = self.client
            .post(&url)
            .json(&serde_json::json!({ "email": email, "password": password }))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .context("Failed to check status")?;

        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::METHOD_NOT_ALLOWED {
            return self.check_status(email);
        }
        if status == reqwest::StatusCode::UNAUTHORIZED {
            anyhow::bail!("Invalid email or password.");
        }
        if !status.is_success() {
            let error_body = response.text().unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Status check failed: {} - {}", status, error_body);
        }

        response.json().context("Failed to parse status response")
    }

    /// Take this device off the account, so its API key stops working. A key the server
    /// no longer knows has got there already.
    pub fn revoke_device(&self, api_key: &str) -> Result<()> {
        let url = format!("{}/api/v1/sync/device", self.base_url);

        let response = self.client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .context("Failed to reach the server")?;

        let status = response.status();
        if !status.is_success() && status != reqwest::StatusCode::UNAUTHORIZED {
            anyhow::bail!("Failed to revoke this device: {}", status);
        }

        Ok(())
    }

    /// Check user approval status (no auth required)
    pub fn check_status(&self, email: &str) -> Result<UserStatusResponse> {
        let url = format!(
            "{}/api/v1/user/status?email={}",
            self.base_url,
            urlencoding::encode(email)
        );

        let response = self.client
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .context("Failed to check status")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Status check failed: {} - {}", status, error_body);
        }

        let result: UserStatusResponse = response.json()
            .context("Failed to parse status response")?;

        Ok(result)
    }
}

/// Wrapped key response from server (envelope encryption)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct WrappedKeyResponse {
    pub blob: String,
    pub kdf_version: u32,
    pub kdf_iterations: u32,
    /// Changes on every write. Older servers don't send it, and it's how a client can
    /// tell that a server takes conditional writes.
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// What a write to the server's wrapped key is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyWrite<'a> {
    /// The account had no key when last read
    First,
    /// Replacing a blob this device read but couldn't open, and no other
    Repair { updated_at: Option<&'a str> },
    /// The password changed, so the blob has to follow it
    PasswordChange,
}

/// How a write to the server's wrapped key went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyWriteResult {
    Stored,
    /// Something else wrote first, and this changed nothing
    Lost,
}

/// Put wrapped key request (envelope encryption)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PutWrappedKeyRequest {
    pub blob: String,
    pub kdf_version: u32,
    pub kdf_iterations: u32,
}

/// Login response for session-based auth
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub session_id: String,
    #[allow(dead_code)]
    pub expires_at: String,
    /// The session can only change the password until this is cleared
    #[serde(default)]
    pub must_change_password: bool,
}

/// Inbox token generation response
#[derive(Debug, Deserialize)]
pub struct InboxTokenResponse {
    pub token: String,
}

/// Inbox token status response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxTokenStatusResponse {
    pub has_token: bool,
}

/// What the server says about an account. It's shown to the user, and acted on no further.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub email: String,
    #[serde(default)]
    pub note_count: i64,
    #[serde(default)]
    pub attachment_count: i64,
    #[serde(default)]
    pub storage_used_bytes: i64,
    #[serde(default)]
    pub storage_quota_mb: i64,
    #[serde(default)]
    pub last_sync_at: Option<String>,
}

/// Session-based API client (for user operations requiring login)
pub struct SessionClient {
    base_url: String,
    session_token: String,
    client: reqwest::blocking::Client,
}

impl SessionClient {
    /// Login and create a session client
    pub fn login(base_url: &str, email: &str, password: &str) -> Result<Self> {
        let client = reqwest::blocking::Client::new();

        // The account route. /auth/login is the admin dashboard's, and refuses everyone else.
        let url = format!("{}/api/v1/user/login", base_url);

        let response = client
            .post(&url)
            .json(&serde_json::json!({
                "email": email,
                "password": password,
            }))
            .send()
            .context("Failed to send login request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Login failed: {} - {}", status, error_body);
        }

        let login_response: LoginResponse = response.json()
            .context("Failed to parse login response")?;

        if login_response.must_change_password {
            anyhow::bail!("{}", rust_i18n::t!("cli.password_change_required"));
        }

        Ok(Self {
            base_url: base_url.to_string(),
            session_token: login_response.session_id,
            client,
        })
    }

    /// What the server holds for this account
    pub fn account_info(&self) -> Result<AccountInfo> {
        let response = self.client
            .get(format!("{}/api/v1/user/account", self.base_url))
            .header("Authorization", format!("Bearer {}", self.session_token))
            .send()
            .context("Failed to reach the server")?;

        if !response.status().is_success() {
            anyhow::bail!("{}", response.status());
        }

        response.json().context("Failed to read the account details")
    }

    /// Delete something of the account's, with the password that confirms it.
    fn delete_with_password(&self, path: &str, password: &str) -> Result<()> {
        let response = self.client
            .delete(format!("{}{path}", self.base_url))
            .header("Authorization", format!("Bearer {}", self.session_token))
            .json(&serde_json::json!({ "currentPassword": password }))
            .send()
            .context("Failed to reach the server")?;

        if !response.status().is_success() {
            anyhow::bail!("{}", response.status());
        }
        Ok(())
    }

    /// Delete every note the server holds for this account
    pub fn delete_all_notes(&self, password: &str) -> Result<()> {
        self.delete_with_password("/api/v1/user/notes", password)
    }

    /// Delete the account and everything the server holds for it
    pub fn delete_account(&self, password: &str) -> Result<()> {
        self.delete_with_password("/api/v1/user/account", password)
    }

    /// Generate a new inbox token
    pub fn generate_inbox_token(&self) -> Result<String> {
        let url = format!("{}/api/v1/user/inbox-token", self.base_url);

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.session_token))
            .send()
            .context("Failed to send inbox token request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to generate inbox token: {} - {}", status, error_body);
        }

        let result: InboxTokenResponse = response.json()
            .context("Failed to parse inbox token response")?;

        Ok(result.token)
    }

    /// Revoke the current inbox token
    pub fn revoke_inbox_token(&self) -> Result<()> {
        let url = format!("{}/api/v1/user/inbox-token", self.base_url);

        let response = self.client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.session_token))
            .send()
            .context("Failed to send revoke request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to revoke inbox token: {} - {}", status, error_body);
        }

        Ok(())
    }

    /// Check if an inbox token exists
    pub fn get_inbox_token_status(&self) -> Result<bool> {
        let url = format!("{}/api/v1/user/inbox-token/status", self.base_url);

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.session_token))
            .send()
            .context("Failed to check inbox token status")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to check token status: {} - {}", status, error_body);
        }

        let result: InboxTokenStatusResponse = response.json()
            .context("Failed to parse token status response")?;

        Ok(result.has_token)
    }
}

/// Delete an inbox item from the server (using device API key auth)
pub fn delete_inbox_item(endpoint: &str, api_key: &str, item_id: &str) -> Result<()> {
    let client = reqwest::blocking::Client::new();
    let url = format!("{}/api/v1/inbox/{}", endpoint, item_id);

    let response = client
        .delete(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .context("Failed to delete inbox item")?;

    if !response.status().is_success() && response.status() != reqwest::StatusCode::NOT_FOUND {
        let error_body = response.text().unwrap_or_else(|_| "Unknown error".to_string());
        anyhow::bail!("Failed to delete inbox item: {}", error_body);
    }

    Ok(())
}

/// Delete all inbox items from the server (using device API key auth)
pub fn delete_all_inbox_items(endpoint: &str, api_key: &str) -> Result<()> {
    let client = reqwest::blocking::Client::new();
    let url = format!("{}/api/v1/inbox", endpoint);

    let response = client
        .delete(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .context("Failed to delete all inbox items")?;

    if !response.status().is_success() {
        let error_body = response.text().unwrap_or_else(|_| "Unknown error".to_string());
        anyhow::bail!("Failed to delete all inbox items: {}", error_body);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Answers one request, and hands back the path it was asked for.
    fn serve_once(body: &'static str) -> (String, std::sync::mpsc::Receiver<String>) {
        use std::io::Write;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (sender, receiver) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let path = crate::test_support::read_request(&mut stream).map(|request| request.path).unwrap_or_default();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len(),
            );
            stream.write_all(response.as_bytes()).unwrap();
            sender.send(path).unwrap();
        });

        (endpoint, receiver)
    }

    #[test]
    fn https_is_always_safe() {
        assert!(is_safe_endpoint("https://notes.example.com").unwrap());
        assert!(is_safe_endpoint("https://203.0.113.5:8443/").unwrap());
    }

    #[test]
    fn plain_http_is_safe_only_off_the_internet() {
        for local in [
            "http://localhost:3030",
            "http://127.0.0.1:3030",
            "http://[::1]:3030",
            "http://10.1.2.3",
            "http://172.16.0.9",
            "http://192.168.1.20:8080",
            "http://169.254.10.1",
            "http://[fd12:3456::1]",
            "http://[fe80::1]",
            "http://[::ffff:192.168.1.20]",
            "http://nas",
            "http://notes.local",
            "http://NOTES.LOCAL:3030",
        ] {
            assert!(is_safe_endpoint(local).unwrap(), "{local}");
        }

        for public in [
            "http://notes.example.com",
            "http://example.com:3030",
            "http://203.0.113.5",
            "http://172.32.0.1",
            "http://[2001:db8::1]",
            "http://[::ffff:203.0.113.5]",
            "http://notes.local.example.com",
        ] {
            assert!(!is_safe_endpoint(public).unwrap(), "{public}");
        }
    }

    #[test]
    fn anything_else_is_not_an_address() {
        assert!(is_safe_endpoint("notes.example.com").is_err());
        assert!(is_safe_endpoint("ftp://notes.example.com").is_err());
        assert!(is_safe_endpoint("").is_err());
    }

    #[test]
    fn a_command_needs_the_flag_for_plain_http_to_a_public_host() {
        assert!(require_safe_endpoint("http://notes.example.com", false).is_err());
        assert!(require_safe_endpoint("http://notes.example.com", true).is_ok());
        assert!(require_safe_endpoint("http://localhost:3030", false).is_ok());
    }

    /// The admin route refuses non-admins, so an ordinary account could never sign in.
    #[test]
    fn signing_in_uses_the_account_route() {
        let (endpoint, path) = serve_once(r#"{"sessionId":"session","expiresAt":"2030-01-01T00:00:00Z"}"#);

        let client = SessionClient::login(&endpoint, "someone@example.com", &uuid::Uuid::new_v4().to_string()).unwrap();

        assert_eq!(path.recv().unwrap(), "/api/v1/user/login");
        assert_eq!(client.session_token, "session");
    }

    #[test]
    fn an_account_that_must_change_its_password_is_told_so() {
        let (endpoint, _path) = serve_once(
            r#"{"sessionId":"session","expiresAt":"2030-01-01T00:00:00Z","mustChangePassword":true}"#,
        );

        let error = SessionClient::login(&endpoint, "someone@example.com", &uuid::Uuid::new_v4().to_string())
            .err().unwrap();

        assert_eq!(error.to_string(), rust_i18n::t!("cli.password_change_required"));
    }

    /// Answers one request per reply given, in order, and hands back each request line.
    fn serve(replies: Vec<(&'static str, &'static str)>) -> (String, std::sync::mpsc::Receiver<String>) {
        use std::io::Write;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (sender, receiver) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            for (status, body) in replies {
                let (mut stream, _) = listener.accept().unwrap();
                let line = crate::test_support::read_request(&mut stream).map(|request| request.line()).unwrap_or_default();
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len(),
                );
                stream.write_all(response.as_bytes()).unwrap();
                sender.send(line).unwrap();
            }
        });

        (endpoint, receiver)
    }

    const APPROVED: &str = r#"{"exists":true,"isApproved":true,"isActive":true}"#;

    #[test]
    fn a_status_check_with_a_password_posts_it() {
        let (endpoint, requests) = serve(vec![("200 OK", APPROVED)]);

        let status = AuthClient::new(endpoint)
            .check_own_status("someone@example.com", &uuid::Uuid::new_v4().to_string())
            .unwrap();

        assert!(status.is_approved);
        assert!(requests.recv().unwrap().starts_with("POST /api/v1/user/status "));
    }

    #[test]
    fn an_older_server_is_asked_the_old_way() {
        for refusal in ["404 Not Found", "405 Method Not Allowed"] {
            let (endpoint, requests) = serve(vec![(refusal, "{}"), ("200 OK", APPROVED)]);

            let status = AuthClient::new(endpoint)
                .check_own_status("someone@example.com", &uuid::Uuid::new_v4().to_string())
                .unwrap();

            assert!(status.is_approved);
            assert!(requests.recv().unwrap().starts_with("POST "));
            assert!(requests.recv().unwrap().starts_with("GET /api/v1/user/status?email="));
        }
    }

    #[test]
    fn a_wrong_password_is_not_answered_the_old_way() {
        let (endpoint, _requests) = serve(vec![("401 Unauthorized", "{}")]);

        assert!(AuthClient::new(endpoint)
            .check_own_status("someone@example.com", &uuid::Uuid::new_v4().to_string())
            .is_err());
    }

    #[test]
    fn test_auth_client_creation() {
        let client = AuthClient::new("http://localhost:3000".to_string());
        assert_eq!(client.base_url, "http://localhost:3000");
    }
}
