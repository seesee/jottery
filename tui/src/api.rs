// API client for Jottery server authentication
// Handles user registration and device registration

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

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

    /// Store or update the server-escrowed wrapped master key (API key auth)
    pub fn put_wrapped_key(&self, api_key: &str, request: &PutWrappedKeyRequest) -> Result<()> {
        let url = format!("{}/api/v1/auth/wrapped-key", self.base_url);

        let response = self.client
            .put(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(request)
            .send()
            .context("Failed to store wrapped key")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to store wrapped key: {} - {}", status, error_body);
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
pub struct WrappedKeyResponse {
    pub blob: String,
    pub kdf_version: u32,
    pub kdf_iterations: u32,
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
        let url = format!("{}/api/v1/auth/login", base_url);

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

        Ok(Self {
            base_url: base_url.to_string(),
            session_token: login_response.session_id,
            client,
        })
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

    #[test]
    fn test_auth_client_creation() {
        let client = AuthClient::new("http://localhost:3000".to_string());
        assert_eq!(client.base_url, "http://localhost:3000");
    }
}
