// API client for Jottery User Portal

const API_BASE = import.meta.env.VITE_API_BASE_URL || '';

interface LoginRequest {
  email: string;
  password: string;
}

interface LoginResponse {
  sessionId: string;
  expiresAt: string;
  user: {
    id: string;
    email: string;
    isAdmin: boolean;
  };
  /// True when the user has ≥ 1 passkey enrolled — the returned
  /// sessionId is MFA-pending and only usable against the passkey
  /// authenticate endpoints until the assertion completes.
  mfaRequired?: boolean;
}

interface Passkey {
  id: string;
  nickname: string | null;
  createdAt: string;
  lastUsedAt: string | null;
}

/// Server-generated PublicKeyCredentialCreationOptions, base64url-encoded
/// where the WebAuthn spec expects `ArrayBuffer`s. Our helpers decode
/// these before invoking navigator.credentials.
interface BeginRegistrationResponse {
  challengeId: string;
  publicKey: PublicKeyCredentialCreationOptionsJSON;
}

interface BeginAuthenticationResponse {
  challengeId: string;
  publicKey: PublicKeyCredentialRequestOptionsJSON;
}

type PublicKeyCredentialCreationOptionsJSON = unknown;
type PublicKeyCredentialRequestOptionsJSON = unknown;

interface InboxAccountInfo {
  itemCount: number;
  totalSizeBytes: number;
  maxItems: number;
  maxSizeMb: number;
  hasToken: boolean;
}

interface UserAccountInfo {
  email: string;
  noteCount: number;
  attachmentCount: number;
  storageUsedBytes: number;
  storageQuotaMb: number;
  createdAt: string;
  lastSyncAt: string | null;
  inbox: InboxAccountInfo;
}

interface InboxTokenResponse {
  token: string;
}

interface Device {
  id: string;
  name: string;
  type: string;
  createdAt: string;
  lastSeenAt: string | null;
  isActive: boolean;
}

interface UserStatusResponse {
  exists: boolean;
  isApproved: boolean;
  isActive: boolean;
}

class ApiError extends Error {
  constructor(
    public status: number,
    public message: string,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

class UserApiClient {
  private sessionToken: string | null = null;

  constructor() {
    // Try to load session token from localStorage (separate from admin token)
    this.sessionToken = localStorage.getItem('user_session_token');
  }

  private async request<T>(
    method: string,
    path: string,
    body?: unknown,
  ): Promise<T> {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
    };

    // Add session token to Authorization header if available
    if (this.sessionToken) {
      headers['Authorization'] = `Bearer ${this.sessionToken}`;
    }

    const response = await fetch(`${API_BASE}${path}`, {
      method,
      headers,
      body: body ? JSON.stringify(body) : undefined,
      credentials: 'include', // Include cookies
    });

    if (!response.ok) {
      const errorData = await response.json().catch(() => ({ error: 'Unknown error' }));
      throw new ApiError(response.status, errorData.error || `HTTP ${response.status}`);
    }

    // Handle 204 No Content
    if (response.status === 204) {
      return undefined as T;
    }

    return response.json();
  }

  // Authentication
  async login(email: string, password: string): Promise<LoginResponse> {
    const response = await this.request<LoginResponse>('POST', '/api/v1/user/login', {
      email,
      password,
    });

    // Store session token
    this.sessionToken = response.sessionId;
    localStorage.setItem('user_session_token', response.sessionId);

    return response;
  }

  async logout(): Promise<void> {
    try {
      await this.request('POST', '/api/v1/user/logout');
    } catch (e) {
      // Ignore errors during logout - we'll clear local state anyway
      console.warn('Logout request failed:', e);
    }
    this.sessionToken = null;
    localStorage.removeItem('user_session_token');
  }

  isAuthenticated(): boolean {
    return this.sessionToken !== null;
  }

  // Account information
  async getAccountInfo(): Promise<UserAccountInfo> {
    return this.request('GET', '/api/v1/user/account');
  }

  // Change password
  async changePassword(currentPassword: string, newPassword: string): Promise<void> {
    return this.request('POST', '/api/v1/user/change-password', {
      currentPassword,
      newPassword,
    });
  }

  // Delete all notes (keep account)
  async deleteAllNotes(): Promise<void> {
    return this.request('DELETE', '/api/v1/user/notes');
  }

  // Deactivate account (can re-register with admin approval)
  async deactivateAccount(): Promise<void> {
    await this.request('DELETE', '/api/v1/user/account?mode=deactivate');
    // Clear local session since account is now inactive
    this.sessionToken = null;
    localStorage.removeItem('user_session_token');
  }

  // Permanently delete account and all data
  async deleteAccount(): Promise<void> {
    await this.request('DELETE', '/api/v1/user/account?mode=delete');
    // Clear local session since account no longer exists
    this.sessionToken = null;
    localStorage.removeItem('user_session_token');
  }

  // Device management
  async getDevices(): Promise<Device[]> {
    return this.request('GET', '/api/v1/user/devices');
  }

  async revokeDevice(deviceId: string): Promise<void> {
    return this.request('DELETE', `/api/v1/user/devices/${deviceId}`);
  }

  // Inbox token management
  async generateInboxToken(): Promise<InboxTokenResponse> {
    return this.request('POST', '/api/v1/user/inbox-token');
  }

  async revokeInboxToken(): Promise<void> {
    return this.request('DELETE', '/api/v1/user/inbox-token');
  }

  async getInboxTokenStatus(): Promise<{ hasToken: boolean }> {
    return this.request('GET', '/api/v1/user/inbox-token/status');
  }

  // -------- Passkeys --------

  async listPasskeys(): Promise<Passkey[]> {
    return this.request('GET', '/api/v1/user/passkeys');
  }

  async deletePasskey(id: string): Promise<void> {
    return this.request('DELETE', `/api/v1/user/passkeys/${encodeURIComponent(id)}`);
  }

  /// Full passkey enrolment flow. Kept on the API client so the Svelte
  /// page can stay thin: begin → invoke the browser → complete.
  /// Returns the newly-created passkey record on success.
  async registerPasskey(nickname: string | null): Promise<Passkey> {
    const { startRegistration } = await import('@simplewebauthn/browser');
    const begin = await this.request<BeginRegistrationResponse>('POST', '/api/v1/user/passkeys/register/begin', {});
    // The server returns the CreationOptions under `publicKey` in the
    // JSON shape @simplewebauthn/browser expects directly.
    const assertion = await startRegistration({ optionsJSON: begin.publicKey as any });
    return this.request<Passkey>('POST', '/api/v1/user/passkeys/register/complete', {
      challengeId: begin.challengeId,
      nickname,
      response: assertion,
    });
  }

  /// Full passkey authentication flow for a session that's just been
  /// issued in MFA-pending state. On success the current session is
  /// promoted to fully verified and protected endpoints will start
  /// accepting it. The caller should re-fetch account info, etc.
  async completePasskeyAuthentication(): Promise<void> {
    const { startAuthentication } = await import('@simplewebauthn/browser');
    const begin = await this.request<BeginAuthenticationResponse>(
      'POST',
      '/api/v1/user/passkeys/authenticate/begin',
      {},
    );
    const assertion = await startAuthentication({ optionsJSON: begin.publicKey as any });
    await this.request('POST', '/api/v1/user/passkeys/authenticate/complete', {
      challengeId: begin.challengeId,
      response: assertion,
    });
  }

  // Check user approval status (no auth required)
  async checkStatus(email: string): Promise<UserStatusResponse> {
    const response = await fetch(`${API_BASE}/api/v1/user/status?email=${encodeURIComponent(email)}`, {
      method: 'GET',
      headers: {
        'Content-Type': 'application/json',
      },
    });

    if (!response.ok) {
      const errorData = await response.json().catch(() => ({ error: 'Unknown error' }));
      throw new ApiError(response.status, errorData.error || `HTTP ${response.status}`);
    }

    return response.json();
  }
}

export const userApi = new UserApiClient();
export { ApiError };
export type { LoginResponse, UserAccountInfo, InboxAccountInfo, InboxTokenResponse, UserStatusResponse, Device, Passkey };
