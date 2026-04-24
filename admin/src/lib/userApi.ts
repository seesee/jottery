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
}

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
  status: 'approved' | 'pending_approval' | 'deactivated';
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

  // Check user approval status. Requires the user's password so the endpoint
  // can't be used to enumerate account existence. 401 on unknown email OR
  // wrong password — both paths take the same time on the server.
  async checkStatus(email: string, password: string): Promise<UserStatusResponse> {
    const response = await fetch(`${API_BASE}/api/v1/user/status`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ email, password }),
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
export type { LoginResponse, UserAccountInfo, InboxAccountInfo, InboxTokenResponse, UserStatusResponse, Device };
