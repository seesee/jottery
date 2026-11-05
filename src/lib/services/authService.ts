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
  /** Opaque. Its presence means the server supports conditional writes */
  updatedAt?: string;
}

export interface PutWrappedKeyRequest {
  blob: string;
  kdfVersion: number;
  kdfIterations: number;
}

/**
 * What a write to the account's key is for. There is one key per account, and
 * overwriting it with the wrong one locks out every new device.
 * - `replace: false`: a first upload, refused if the account already has a key.
 * - `replace: true`: a password change, which always applies.
 * - `ifMatch`: a repair, refused if the key has changed since it was read.
 */
export type WrappedKeyWrite = { replace: boolean } | { ifMatch: string };

/** A refusal from the server, as opposed to not reaching it at all */
export class AuthApiError extends Error {
  constructor(message: string, public readonly status: number) {
    super(message);
    this.name = 'AuthApiError';
  }
}

/** A header value, as the server is untrusted and fetch throws on a bad one */
const UPDATED_AT_PATTERN = /^[\x20-\x7e]{1,256}$/;

/**
 * The wait a rate limit asks for, in the server's own words. Those are shown as
 * text and limited in length, because the server is untrusted.
 */
function rateLimitMessage(errorText: string): string {
  try {
    const { error } = JSON.parse(errorText);
    if (typeof error === 'string' && error.length > 0 && error.length <= 200) {
      return error;
    }
  } catch {
    // Not JSON, so fall through to the general message
  }
  return 'Too many attempts. Please wait and try again.';
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
      if (response.status === 429) {
        throw new AuthApiError(rateLimitMessage(errorText), 429);
      }
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
        throw new AuthApiError(
          'Your account is pending admin approval or has been deactivated. Please contact the administrator.',
          status
        );
      } else if (status === 401) {
        throw new AuthApiError('Invalid email or password.', status);
      } else if (status === 429) {
        throw new AuthApiError(rateLimitMessage(errorText), status);
      } else {
        throw new AuthApiError(`Device registration failed: ${errorText}`, status);
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
        throw new AuthApiError(
          'Your account is pending admin approval or has been deactivated. Please contact the administrator.',
          status
        );
      } else if (status === 401) {
        throw new AuthApiError('Invalid API key or device is inactive.', status);
      } else if (status === 429) {
        throw new AuthApiError(rateLimitMessage(errorText), status);
      } else {
        throw new AuthApiError(`Device cloning failed: ${errorText}`, status);
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

    const wrappedKey: WrappedKeyResponse = await response.json();
    return {
      ...wrappedKey,
      updatedAt: typeof wrappedKey.updatedAt === 'string' && UPDATED_AT_PATTERN.test(wrappedKey.updatedAt)
        ? wrappedKey.updatedAt
        : undefined,
    };
  }

  /**
   * Store or update the server-escrowed wrapped master key.
   *
   * @throws AuthApiError with status 409 if the write's condition failed. Nothing was
   * written. Don't retry it: read the key again and decide again from what's there.
   */
  async putWrappedKey(
    endpoint: string,
    authToken: string,
    request: PutWrappedKeyRequest,
    write: WrappedKeyWrite
  ): Promise<void> {
    endpoint = this.normalizeEndpoint(endpoint);
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${authToken}`,
    };
    if ('ifMatch' in write) {
      headers['If-Match'] = write.ifMatch;
    }

    const response = await fetch(`${endpoint}/api/v1/auth/wrapped-key`, {
      method: 'PUT',
      headers,
      body: JSON.stringify('replace' in write ? { ...request, replace: write.replace } : request),
    });

    if (!response.ok) {
      throw new AuthApiError(`Failed to store wrapped key: ${await response.text()}`, response.status);
    }
  }
}

export const authService = new AuthService();
