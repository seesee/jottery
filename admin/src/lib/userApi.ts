// API client for Jottery User Portal

import { ApiError, DashboardClient } from './dashboardClient';
import type { RequestOptions } from './dashboardClient';

const API_BASE = import.meta.env.VITE_API_BASE_URL || '';

interface LoginRequest {
  email: string;
  password: string;
}

interface LoginResponse {
  // Absent when the server keeps the session in an HttpOnly cookie
  sessionId?: string;
  cookieSession?: boolean;
  expiresAt: string;
  user: {
    id: string;
    email: string;
    isAdmin: boolean;
  };
  // Older servers don't send this
  mustChangePassword?: boolean;
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
  exists: boolean;
  isApproved: boolean;
  isActive: boolean;
}

class UserApiClient {
  // Separate from the admin session
  private client = new DashboardClient('user_session_token');

  private get session() {
    return this.client.session;
  }

  private request<T>(method: string, path: string, body?: unknown, options?: RequestOptions): Promise<T> {
    return this.client.request<T>(method, path, body, options);
  }

  onSessionExpired(listener: () => void): void {
    this.client.onSessionExpired(listener);
  }

  onPasswordChangeRequired(listener: () => void): void {
    this.client.onPasswordChangeRequired(listener);
  }

  // Authentication
  async login(email: string, password: string): Promise<LoginResponse> {
    const response = await this.request<LoginResponse>('POST', '/api/v1/user/login', {
      email,
      password,
      // Servers without cookie sessions ignore this and return a token
      cookieSession: true,
    }, { checksPassword: true });

    this.session.start(response);

    return response;
  }

  async logout(): Promise<void> {
    try {
      await this.request('POST', '/api/v1/user/logout');
    } catch (e) {
      // Ignore errors during logout - we'll clear local state anyway
      console.warn('Logout request failed:', e);
    }
    this.session.clear();
  }

  isAuthenticated(): boolean {
    return this.session.isActive;
  }

  // Account information
  async getAccountInfo(): Promise<UserAccountInfo> {
    return this.request('GET', '/api/v1/user/account');
  }

  /** Who is signed in, which also shows whether the session still works. */
  async currentUser(): Promise<{ email: string }> {
    return this.getAccountInfo();
  }

  // Change password
  async changePassword(currentPassword: string, newPassword: string): Promise<void> {
    return this.request('POST', '/api/v1/user/change-password', {
      currentPassword,
      newPassword,
    }, { checksPassword: true });
  }

  // Delete all notes (keep account). Older servers ignore the password. Current ones
  // refuse these three requests without it, unless the session is minutes old.
  async deleteAllNotes(currentPassword: string): Promise<void> {
    return this.request('DELETE', '/api/v1/user/notes', { currentPassword }, { checksPassword: true });
  }

  // Deactivate account (can re-register with admin approval)
  async deactivateAccount(currentPassword: string): Promise<void> {
    await this.request('DELETE', '/api/v1/user/account?mode=deactivate', { currentPassword }, { checksPassword: true });
    // Clear local session since account is now inactive
    this.session.clear();
  }

  // Permanently delete account and all data
  async deleteAccount(currentPassword: string): Promise<void> {
    await this.request('DELETE', '/api/v1/user/account?mode=delete', { currentPassword }, { checksPassword: true });
    // Clear local session since account no longer exists
    this.session.clear();
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

  /**
   * Where an account stands, for someone who holds its password. A pending account
   * can't sign in, so this is how its owner finds out. Older servers only have the
   * GET, which answers for any address and doesn't need the password.
   */
  async checkStatus(email: string, password: string): Promise<UserStatusResponse> {
    try {
      return await this.request<UserStatusResponse>(
        'POST',
        '/api/v1/user/status',
        { email, password },
        { checksPassword: true },
      );
    } catch (err) {
      if (err instanceof ApiError && (err.status === 404 || err.status === 405)) {
        return this.checkStatusWithoutPassword(email);
      }
      throw err;
    }
  }

  private async checkStatusWithoutPassword(email: string): Promise<UserStatusResponse> {
    const response = await fetch(`${API_BASE}/api/v1/user/status?email=${encodeURIComponent(email)}`, {
      method: 'GET',
      headers: {
        'Content-Type': 'application/json',
      },
    });

    if (!response.ok) {
      const errorData = await response.json().catch(() => null);
      throw new ApiError(response.status, errorData?.error ?? '');
    }

    return response.json();
  }
}

export const userApi = new UserApiClient();
export { ApiError };
export type { LoginResponse, UserAccountInfo, InboxAccountInfo, InboxTokenResponse, UserStatusResponse, Device };
