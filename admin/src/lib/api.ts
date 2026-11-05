// API client for Jottery Admin Dashboard

import { ApiError, DashboardClient } from './dashboardClient';
import type { RequestOptions } from './dashboardClient';

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

interface UserListItem {
  id: string;
  email: string;
  approved: boolean;
  isAdmin: boolean;
  isActive: boolean;
  createdAt: string;
  deviceCount: number;
  noteCount: number;
}

interface StatsResponse {
  users: {
    total: number;
    approved: number;
    pending: number;
    active: number;
  };
  devices: {
    total: number;
    active: number;
  };
  notes: {
    total: number;
  };
  storage: {
    totalBytes: number;
    quotaBytes: number;
  };
}

interface Device {
  id: string;
  name: string;
  type: string;
  createdAt: string;
  lastSeenAt: string | null;
  isActive: boolean;
}

interface UserDetail {
  id: string;
  email: string;
  approved: boolean;
  isAdmin: boolean;
  isActive: boolean;
  createdAt: string;
  approvedAt: string | null;
  lastLoginAt: string | null;
  storageQuotaMb: number;
  maxUploadSizeMb: number;
  inboxMaxItems?: number;
  inboxMaxSizeMb?: number;
  stats: {
    devices: {
      total: number;
      active: number;
      lastSeen: string | null;
    };
    notes: {
      count: number;
    };
    attachments: {
      count: number;
      totalBytes: number;
    };
    inbox: {
      count: number;
      totalBytes: number;
      hasToken: boolean;
    };
    lastSyncAt: string | null;
  };
  devices: Device[];
}

interface AuditEntry {
  id: string;
  // Null for actions taken with jottery-admin, which has no account
  adminUserId: string | null;
  action: string;
  targetUserId: string | null;
  targetResourceId: string | null;
  createdAt: string;
  ipAddress: string | null;
  userAgent: string | null;
  metadata: unknown;
}

interface UpdateUserSettingsRequest {
  storageQuotaMb?: number;
  maxUploadSizeMb?: number;
  inboxMaxItems?: number;
  inboxMaxSizeMb?: number;
}

class ApiClient {
  private client = new DashboardClient('admin_session_token');

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
    const response = await this.request<LoginResponse>('POST', '/api/v1/auth/login', {
      email,
      password,
      // Servers without cookie sessions ignore this and return a token
      cookieSession: true,
    }, { checksPassword: true });

    this.session.start(response);

    return response;
  }

  async logout(): Promise<void> {
    // End the session on the server, or a stolen token would stay valid until it
    // expires. Admin cookies are scoped to the admin API, so they need the admin
    // route. A token works on the user route, which every server version has.
    const path = this.session.usesCookie ? '/api/v1/admin/logout' : '/api/v1/user/logout';
    try {
      await this.request('POST', path);
    } catch (e) {
      // Clear local state regardless
      console.warn('Logout request failed:', e);
    }
    this.session.clear();
  }

  isAuthenticated(): boolean {
    return this.session.isActive;
  }

  /** Who is signed in, which also shows whether the session still works. */
  async currentUser(): Promise<{ email: string }> {
    // As with logout: a token works on the user route, which every server version has
    const path = this.session.usesCookie ? '/api/v1/admin/me' : '/api/v1/user/account';
    return this.request('GET', path);
  }

  // Admin - Users
  async listUsers(): Promise<{ users: UserListItem[]; total: number }> {
    return this.request('GET', '/api/v1/admin/users');
  }

  async getUser(id: string): Promise<UserDetail> {
    return this.request('GET', `/api/v1/admin/users/${id}`);
  }

  async getUserDevices(userId: string): Promise<{ devices: Device[] }> {
    return this.request('GET', `/api/v1/admin/users/${userId}/devices`);
  }

  async revokeDevice(deviceId: string): Promise<void> {
    return this.request('DELETE', `/api/v1/admin/devices/${deviceId}`);
  }

  async renameDevice(deviceId: string, name: string): Promise<void> {
    return this.request('PATCH', `/api/v1/admin/devices/${deviceId}`, { name });
  }

  async approveUser(id: string): Promise<void> {
    return this.request('POST', `/api/v1/admin/users/${id}/approve`);
  }

  async deactivateUser(id: string): Promise<void> {
    return this.request('POST', `/api/v1/admin/users/${id}/deactivate`);
  }

  async activateUser(id: string): Promise<void> {
    return this.request('POST', `/api/v1/admin/users/${id}/activate`);
  }

  async deleteUser(id: string): Promise<void> {
    return this.request('DELETE', `/api/v1/admin/users/${id}`);
  }

  async toggleAdmin(id: string, isAdmin: boolean): Promise<void> {
    return this.request('POST', `/api/v1/admin/users/${id}/toggle-admin`, {
      is_admin: isAdmin,
    });
  }

  async updateUserSettings(id: string, settings: UpdateUserSettingsRequest): Promise<void> {
    return this.request('PATCH', `/api/v1/admin/users/${id}/settings`, settings);
  }

  async resetUserPassword(id: string, newPassword: string, revokeDevices: boolean): Promise<void> {
    return this.request('POST', `/api/v1/admin/users/${id}/reset-password`, {
      new_password: newPassword,
      revokeDevices,
    });
  }

  // Admin - Stats
  async getStats(): Promise<StatsResponse> {
    return this.request('GET', '/api/v1/admin/stats');
  }

  // Admin - Audit log, newest first
  async getAuditLog(query: { limit: number; offset: number; action?: string }): Promise<{ logs: AuditEntry[]; total: number }> {
    const params = new URLSearchParams({ limit: String(query.limit), offset: String(query.offset) });
    if (query.action) {
      params.set('action', query.action);
    }
    return this.request('GET', `/api/v1/admin/audit?${params}`);
  }

  // Admin - Password
  async changePassword(currentPassword: string, newPassword: string): Promise<void> {
    return this.request('POST', '/api/v1/admin/change-password', {
      current_password: currentPassword,
      new_password: newPassword,
    }, { checksPassword: true });
  }
}

export const api = new ApiClient();
export { ApiError };
export type { LoginResponse, UserListItem, UserDetail, Device, StatsResponse, AuditEntry };
