/**
 * Authentication service for multi-user registration
 * Handles user account creation and device registration
 */

// User registration types
export interface RegisterUserRequest {
  email: string;
  password: string;
}

export interface RegisterUserResponse {
  userId: string;
  email: string;
  status: string; // 'pending_approval' or 'approved'
  message: string;
}

// Device registration types
export interface RegisterDeviceRequest {
  email: string;
  password: string;
  deviceName: string;
  deviceType: string;
}

export interface RegisterDeviceResponse {
  clientId: string;
  apiKey: string;
  userId: string;
  deviceName: string;
}

// Clone device types (for importing credentials)
export interface CloneDeviceRequest {
  apiKey: string;
  deviceName: string;
  deviceType: string;
}

export type CloneDeviceResponse = RegisterDeviceResponse;

// Wrapped key types (envelope encryption)
export interface WrappedKeyResponse {
  blob: string;
  kdfVersion: number;
  kdfIterations: number;
}

export interface PutWrappedKeyRequest {
  blob: string;
  kdfVersion: number;
  kdfIterations: number;
}

class AuthService {
  /**
   * Normalize endpoint URL by removing trailing slash
   */
  private normalizeEndpoint(endpoint: string): string {
    return endpoint.endsWith('/') ? endpoint.slice(0, -1) : endpoint;
  }

  /**
   * Check if server is reachable
   */
  async healthCheck(endpoint: string): Promise<boolean> {
    try {
      endpoint = this.normalizeEndpoint(endpoint);
      const response = await fetch(`${endpoint}/health`, {
        method: 'GET',
      });
      return response.ok;
    } catch (error) {
      console.error('[AuthService] Health check failed:', error);
      return false;
    }
  }

  /**
   * Register a new user account
   * Returns pending approval status
   */
  async registerUser(
    endpoint: string,
    email: string,
    password: string
  ): Promise<RegisterUserResponse> {
    endpoint = this.normalizeEndpoint(endpoint);
    const url = `${endpoint}/api/v1/auth/register-user`;

    const request: RegisterUserRequest = {
      email,
      password,
    };

    const response = await fetch(url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(`User registration failed: ${errorText}`);
    }

    const result: RegisterUserResponse = await response.json();
    return result;
  }

  /**
   * Register a device for an existing user
   * Requires user email and password
   * Returns API key and client ID for sync
   */
  async registerDevice(
    endpoint: string,
    email: string,
    password: string,
    deviceName: string
  ): Promise<RegisterDeviceResponse> {
    endpoint = this.normalizeEndpoint(endpoint);
    const url = `${endpoint}/api/v1/auth/register-device`;

    const request: RegisterDeviceRequest = {
      email,
      password,
      deviceName,
      deviceType: 'web',
    };

    const response = await fetch(url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      const status = response.status;
      const errorText = await response.text();

      // Provide user-friendly error messages
      if (status === 403) {
        throw new Error(
          'Your account is pending admin approval or has been deactivated. Please contact the administrator.'
        );
      } else if (status === 401) {
        throw new Error('Invalid email or password.');
      } else {
        throw new Error(`Device registration failed: ${errorText}`);
      }
    }

    const result: RegisterDeviceResponse = await response.json();
    return result;
  }

  /**
   * Clone a device using an existing API key
   * Creates a new device entry for the same user with a new client ID and API key
   * Used when importing sync credentials on a new device
   */
  async cloneDevice(
    endpoint: string,
    apiKey: string,
    deviceName: string
  ): Promise<CloneDeviceResponse> {
    endpoint = this.normalizeEndpoint(endpoint);
    const url = `${endpoint}/api/v1/auth/clone-device`;

    const request: CloneDeviceRequest = {
      apiKey,
      deviceName,
      deviceType: 'web',
    };

    const response = await fetch(url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      const status = response.status;
      const errorText = await response.text();

      if (status === 403) {
        throw new Error(
          'Your account is pending admin approval or has been deactivated. Please contact the administrator.'
        );
      } else if (status === 401) {
        throw new Error('Invalid API key or device is inactive.');
      } else {
        throw new Error(`Device cloning failed: ${errorText}`);
      }
    }

    const result: CloneDeviceResponse = await response.json();
    return result;
  }

  /**
   * Get the server-escrowed wrapped master key for the current user.
   * Returns null if no wrapped key exists (user hasn't migrated to envelope encryption).
   */
  async getWrappedKey(
    endpoint: string,
    authToken: string
  ): Promise<WrappedKeyResponse | null> {
    endpoint = this.normalizeEndpoint(endpoint);
    const response = await fetch(`${endpoint}/api/v1/auth/wrapped-key`, {
      method: 'GET',
      headers: {
        Authorization: `Bearer ${authToken}`,
      },
    });

    if (response.status === 404) {
      return null;
    }

    if (!response.ok) {
      throw new Error(`Failed to get wrapped key: ${await response.text()}`);
    }

    return await response.json();
  }

  /**
   * Store or update the server-escrowed wrapped master key.
   */
  async putWrappedKey(
    endpoint: string,
    authToken: string,
    request: PutWrappedKeyRequest
  ): Promise<void> {
    endpoint = this.normalizeEndpoint(endpoint);
    const response = await fetch(`${endpoint}/api/v1/auth/wrapped-key`, {
      method: 'PUT',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${authToken}`,
      },
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      throw new Error(`Failed to store wrapped key: ${await response.text()}`);
    }
  }
}

export const authService = new AuthService();
