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
