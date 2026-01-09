// Tests for user API client
import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';

// Mock localStorage and fetch globally before any imports
let localStorageStore: Record<string, string> = {};
const localStorageMock = {
  getItem: (key: string) => localStorageStore[key] || null,
  setItem: (key: string, value: string) => {
    localStorageStore[key] = value;
  },
  removeItem: (key: string) => {
    delete localStorageStore[key];
  },
  clear: () => {
    localStorageStore = {};
  },
  length: 0,
  key: () => null,
};

const mockFetch = vi.fn();

// Apply mocks immediately at module level
vi.stubGlobal('localStorage', localStorageMock);
vi.stubGlobal('fetch', mockFetch);

describe('UserApiClient', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorageStore = {};
    // Re-stub after clearing
    vi.stubGlobal('localStorage', localStorageMock);
    vi.stubGlobal('fetch', mockFetch);
  });

  afterEach(() => {
    vi.resetModules();
  });

  // Helper to create a fresh UserApiClient for each test
  async function createFreshApiClient() {
    vi.resetModules();
    vi.stubGlobal('localStorage', localStorageMock);
    vi.stubGlobal('fetch', mockFetch);
    const module = await import('./userApi');
    return module.userApi;
  }

  describe('login', () => {
    it('sends login request with email and password', async () => {
      const userApi = await createFreshApiClient();

      mockFetch.mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: () =>
          Promise.resolve({
            sessionId: 'test-session-id',
            expiresAt: '2025-01-20T00:00:00Z',
            user: { id: '1', email: 'test@example.com', isAdmin: false },
          }),
      });

      const result = await userApi.login('test@example.com', 'password123');

      expect(mockFetch).toHaveBeenCalledWith(
        '/api/v1/user/login',
        expect.objectContaining({
          method: 'POST',
          headers: expect.objectContaining({
            'Content-Type': 'application/json',
          }),
          body: JSON.stringify({ email: 'test@example.com', password: 'password123' }),
        }),
      );

      expect(result.sessionId).toBe('test-session-id');
      expect(result.user.email).toBe('test@example.com');
    });

    it('stores session token in localStorage after login', async () => {
      const userApi = await createFreshApiClient();

      mockFetch.mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: () =>
          Promise.resolve({
            sessionId: 'new-session-token',
            expiresAt: '2025-01-20T00:00:00Z',
            user: { id: '1', email: 'test@example.com', isAdmin: false },
          }),
      });

      await userApi.login('test@example.com', 'password123');

      expect(localStorageStore['user_session_token']).toBe('new-session-token');
    });

    it('throws ApiError on failed login', async () => {
      const userApi = await createFreshApiClient();
      const { ApiError } = await import('./userApi');

      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 401,
        json: () => Promise.resolve({ error: 'Invalid credentials' }),
      });

      await expect(userApi.login('test@example.com', 'wrong')).rejects.toThrow(ApiError);
    });
  });

  describe('logout', () => {
    it('clears session token from localStorage', async () => {
      localStorageStore['user_session_token'] = 'existing-token';
      const userApi = await createFreshApiClient();

      mockFetch.mockResolvedValueOnce({
        ok: true,
        status: 204,
      });

      await userApi.logout();

      expect(localStorageStore['user_session_token']).toBeUndefined();
    });

    it('clears token even if API request fails', async () => {
      localStorageStore['user_session_token'] = 'existing-token';
      const userApi = await createFreshApiClient();

      mockFetch.mockRejectedValueOnce(new Error('Network error'));

      await userApi.logout();

      expect(localStorageStore['user_session_token']).toBeUndefined();
    });
  });

  describe('isAuthenticated', () => {
    it('returns true when session token exists', async () => {
      localStorageStore['user_session_token'] = 'some-token';
      const userApi = await createFreshApiClient();

      expect(userApi.isAuthenticated()).toBe(true);
    });

    it('returns false when no session token', async () => {
      const userApi = await createFreshApiClient();

      expect(userApi.isAuthenticated()).toBe(false);
    });
  });

  describe('getAccountInfo', () => {
    it('fetches account information with auth header', async () => {
      localStorageStore['user_session_token'] = 'auth-token';
      const userApi = await createFreshApiClient();

      mockFetch.mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: () =>
          Promise.resolve({
            email: 'test@example.com',
            noteCount: 10,
            attachmentCount: 5,
            storageUsedBytes: 1024,
            storageQuotaMb: 100,
            createdAt: '2024-01-01T00:00:00Z',
            lastSyncAt: null,
          }),
      });

      const info = await userApi.getAccountInfo();

      expect(mockFetch).toHaveBeenCalledWith(
        '/api/v1/user/account',
        expect.objectContaining({
          method: 'GET',
          headers: expect.objectContaining({
            Authorization: 'Bearer auth-token',
          }),
        }),
      );

      expect(info.email).toBe('test@example.com');
      expect(info.noteCount).toBe(10);
    });
  });

  describe('changePassword', () => {
    it('sends password change request', async () => {
      localStorageStore['user_session_token'] = 'auth-token';
      const userApi = await createFreshApiClient();

      mockFetch.mockResolvedValueOnce({
        ok: true,
        status: 204,
      });

      await userApi.changePassword('oldpass', 'newpass123456');

      expect(mockFetch).toHaveBeenCalledWith(
        '/api/v1/user/change-password',
        expect.objectContaining({
          method: 'POST',
          body: JSON.stringify({ currentPassword: 'oldpass', newPassword: 'newpass123456' }),
        }),
      );
    });

    it('throws ApiError on wrong current password', async () => {
      localStorageStore['user_session_token'] = 'auth-token';
      const userApi = await createFreshApiClient();
      const { ApiError } = await import('./userApi');

      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 401,
        json: () => Promise.resolve({ error: 'Invalid current password' }),
      });

      await expect(userApi.changePassword('wrong', 'newpass')).rejects.toThrow(ApiError);
    });
  });

  describe('deactivateAccount', () => {
    it('sends deactivate request and clears session', async () => {
      localStorageStore['user_session_token'] = 'auth-token';
      const userApi = await createFreshApiClient();

      mockFetch.mockResolvedValueOnce({
        ok: true,
        status: 204,
      });

      await userApi.deactivateAccount();

      expect(mockFetch).toHaveBeenCalledWith(
        '/api/v1/user/account?mode=deactivate',
        expect.objectContaining({
          method: 'DELETE',
        }),
      );

      expect(localStorageStore['user_session_token']).toBeUndefined();
    });
  });

  describe('deleteAccount', () => {
    it('sends delete request and clears session', async () => {
      localStorageStore['user_session_token'] = 'auth-token';
      const userApi = await createFreshApiClient();

      mockFetch.mockResolvedValueOnce({
        ok: true,
        status: 204,
      });

      await userApi.deleteAccount();

      expect(mockFetch).toHaveBeenCalledWith(
        '/api/v1/user/account?mode=delete',
        expect.objectContaining({
          method: 'DELETE',
        }),
      );

      expect(localStorageStore['user_session_token']).toBeUndefined();
    });
  });
});

describe('ApiError', () => {
  it('creates error with status and message', async () => {
    const { ApiError } = await import('./userApi');
    const error = new ApiError(401, 'Unauthorized');

    expect(error.status).toBe(401);
    expect(error.message).toBe('Unauthorized');
    expect(error.name).toBe('ApiError');
  });

  it('is instanceof Error', async () => {
    const { ApiError } = await import('./userApi');
    const error = new ApiError(500, 'Server error');

    expect(error).toBeInstanceOf(Error);
    expect(error).toBeInstanceOf(ApiError);
  });
});
