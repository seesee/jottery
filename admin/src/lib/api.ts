// API client for Jottery Admin Dashboard

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

interface UpdateUserSettingsRequest {
  storageQuotaMb?: number;
  maxUploadSizeMb?: number;
  inboxMaxItems?: number;
  inboxMaxSizeMb?: number;
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

class ApiClient {
  private sessionToken: string | null = null;

  constructor() {
    // Try to load session token from localStorage
    this.sessionToken = localStorage.getItem('admin_session_token');
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
    const response = await this.request<LoginResponse>('POST', '/api/v1/auth/login', {
      email,
      password,
    });

    // Store session token
    this.sessionToken = response.sessionId;
    localStorage.setItem('admin_session_token', response.sessionId);

    return response;
  }

  logout(): void {
    this.sessionToken = null;
    localStorage.removeItem('admin_session_token');
  }

  isAuthenticated(): boolean {
    return this.sessionToken !== null;
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

  async resetUserPassword(id: string, newPassword: string): Promise<void> {
    return this.request('POST', `/api/v1/admin/users/${id}/reset-password`, {
      new_password: newPassword,
    });
  }

  // Admin - Stats
  async getStats(): Promise<StatsResponse> {
    return this.request('GET', '/api/v1/admin/stats');
  }

  // Admin - Password
  async changePassword(currentPassword: string, newPassword: string): Promise<void> {
    return this.request('POST', '/api/v1/admin/change-password', {
      current_password: currentPassword,
      new_password: newPassword,
    });
  }
}

export const api = new ApiClient();
export { ApiError };
export type { LoginResponse, UserListItem, UserDetail, Device, StatsResponse };
