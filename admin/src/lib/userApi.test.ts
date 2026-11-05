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

// The session lives in sessionStorage. localStorage is shared with the notes app
// on the same origin, so it's a separate mock, to prove nothing goes there.
let sessionStore: Record<string, string> = {};
const sessionStorageMock = {
  getItem: (key: string) => sessionStore[key] || null,
  setItem: (key: string, value: string) => {
    sessionStore[key] = value;
  },
  removeItem: (key: string) => {
    delete sessionStore[key];
  },
  clear: () => {
    sessionStore = {};
  },
  length: 0,
  key: () => null,
};

const mockFetch = vi.fn();

// Apply mocks immediately at module level
vi.stubGlobal('localStorage', localStorageMock);
vi.stubGlobal('sessionStorage', sessionStorageMock);
vi.stubGlobal('fetch', mockFetch);

describe('UserApiClient', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorageStore = {};
    sessionStore = {};
    // Re-stub after clearing
    vi.stubGlobal('localStorage', localStorageMock);
    vi.stubGlobal('sessionStorage', sessionStorageMock);
    vi.stubGlobal('fetch', mockFetch);
  });

  afterEach(() => {
    vi.resetModules();
  });

  // Helper to create a fresh UserApiClient for each test
  async function createFreshApiClient() {
    vi.resetModules();
    vi.stubGlobal('localStorage', localStorageMock);
    vi.stubGlobal('sessionStorage', sessionStorageMock);
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
          body: JSON.stringify({ email: 'test@example.com', password: 'password123', cookieSession: true }),
        }),
      );

      expect(result.sessionId).toBe('test-session-id');
      expect(result.user.email).toBe('test@example.com');
    });

    it('stores session token in sessionStorage after login', async () => {
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

      expect(sessionStore['user_session_token']).toBe('new-session-token');
      expect(localStorageStore['user_session_token']).toBeUndefined();
    });

    it('removes a token left in localStorage by an earlier version and ignores it', async () => {
      localStorageStore['user_session_token'] = 'stale-token';

      const userApi = await createFreshApiClient();

      expect(localStorageStore['user_session_token']).toBeUndefined();
      expect(userApi.isAuthenticated()).toBe(false);
    });

    it('keeps no token when the server sets a cookie session', async () => {
      const userApi = await createFreshApiClient();

      mockFetch.mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: () =>
          Promise.resolve({
            cookieSession: true,
            expiresAt: '2025-01-20T00:00:00Z',
            user: { id: '1', email: 'test@example.com', isAdmin: false },
          }),
      });
      await userApi.login('test@example.com', 'password123');

      expect(userApi.isAuthenticated()).toBe(true);
      expect(JSON.parse(mockFetch.mock.calls[0][1].body).cookieSession).toBe(true);

      mockFetch.mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: () => Promise.resolve([]),
      });
      await userApi.getDevices();

      const headers = mockFetch.mock.calls[1][1].headers;
      expect(headers['Authorization']).toBeUndefined();
      expect(headers['X-Requested-With']).toBe('JotteryDashboard');
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
    it('clears session token from sessionStorage', async () => {
      sessionStore['user_session_token'] = 'existing-token';
      const userApi = await createFreshApiClient();

      mockFetch.mockResolvedValueOnce({
        ok: true,
        status: 204,
      });

      await userApi.logout();

      expect(sessionStore['user_session_token']).toBeUndefined();
    });

    it('clears token even if API request fails', async () => {
      sessionStore['user_session_token'] = 'existing-token';
      const userApi = await createFreshApiClient();

      mockFetch.mockRejectedValueOnce(new Error('Network error'));

      await userApi.logout();

      expect(sessionStore['user_session_token']).toBeUndefined();
    });
  });

  describe('isAuthenticated', () => {
    it('returns true when session token exists', async () => {
      sessionStore['user_session_token'] = 'some-token';
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
      sessionStore['user_session_token'] = 'auth-token';
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
      sessionStore['user_session_token'] = 'auth-token';
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
      sessionStore['user_session_token'] = 'auth-token';
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
      sessionStore['user_session_token'] = 'auth-token';
      const userApi = await createFreshApiClient();

      mockFetch.mockResolvedValueOnce({
        ok: true,
        status: 204,
      });

      await userApi.deactivateAccount('the-password');

      expect(mockFetch).toHaveBeenCalledWith(
        '/api/v1/user/account?mode=deactivate',
        expect.objectContaining({
          method: 'DELETE',
          body: JSON.stringify({ currentPassword: 'the-password' }),
        }),
      );

      expect(sessionStore['user_session_token']).toBeUndefined();
    });
  });

  describe('deleteAccount', () => {
    it('sends delete request and clears session', async () => {
      sessionStore['user_session_token'] = 'auth-token';
      const userApi = await createFreshApiClient();

      mockFetch.mockResolvedValueOnce({
        ok: true,
        status: 204,
      });

      await userApi.deleteAccount('the-password');

      expect(mockFetch).toHaveBeenCalledWith(
        '/api/v1/user/account?mode=delete',
        expect.objectContaining({
          method: 'DELETE',
          body: JSON.stringify({ currentPassword: 'the-password' }),
        }),
      );

      expect(sessionStore['user_session_token']).toBeUndefined();
    });

    it('keeps the session when the password is refused', async () => {
      sessionStore['user_session_token'] = 'auth-token';
      const userApi = await createFreshApiClient();

      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 401,
        json: () => Promise.resolve({ error: 'Unauthorized' }),
      });

      await expect(userApi.deleteAccount('wrong')).rejects.toMatchObject({ status: 401 });
      expect(sessionStore['user_session_token']).toBe('auth-token');
    });
  });

  describe('checkStatus', () => {
    const body = { exists: true, isApproved: false, isActive: true };

    it('proves who is asking with the password', async () => {
      const userApi = await createFreshApiClient();
      mockFetch.mockResolvedValueOnce({ ok: true, status: 200, json: () => Promise.resolve(body) });

      await expect(userApi.checkStatus('me@example.com', 'the-password')).resolves.toEqual(body);

      expect(mockFetch).toHaveBeenCalledTimes(1);
      expect(mockFetch).toHaveBeenCalledWith(
        '/api/v1/user/status',
        expect.objectContaining({
          method: 'POST',
          body: JSON.stringify({ email: 'me@example.com', password: 'the-password' }),
        }),
      );
    });

    for (const status of [404, 405]) {
      it(`asks the old way on a server that answers ${status}, without sending the password`, async () => {
        const userApi = await createFreshApiClient();
        mockFetch
          .mockResolvedValueOnce({ ok: false, status, json: () => Promise.reject(new Error('no body')) })
          .mockResolvedValueOnce({ ok: true, status: 200, json: () => Promise.resolve(body) });

        await expect(userApi.checkStatus('me+x@example.com', 'the-password')).resolves.toEqual(body);

        const [url, options] = mockFetch.mock.calls[1];
        expect(url).toBe('/api/v1/user/status?email=me%2Bx%40example.com');
        expect(options.method).toBe('GET');
        expect(url + JSON.stringify(options)).not.toContain('the-password');
      });
    }

    it('reports a wrong password, and doesn\'t fall back to asking without one', async () => {
      const userApi = await createFreshApiClient();
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 401,
        json: () => Promise.resolve({ error: 'Unauthorized' }),
      });

      await expect(userApi.checkStatus('me@example.com', 'wrong')).rejects.toMatchObject({ status: 401 });
      expect(mockFetch).toHaveBeenCalledTimes(1);
    });
  });

  describe('deleteAllNotes', () => {
    it('sends the password in the body', async () => {
      sessionStore['user_session_token'] = 'auth-token';
      const userApi = await createFreshApiClient();

      mockFetch.mockResolvedValueOnce({ ok: true, status: 204 });

      await userApi.deleteAllNotes('the-password');

      expect(mockFetch).toHaveBeenCalledWith(
        '/api/v1/user/notes',
        expect.objectContaining({
          method: 'DELETE',
          body: JSON.stringify({ currentPassword: 'the-password' }),
        }),
      );
    });

    it('keeps the session when the password is refused', async () => {
      sessionStore['user_session_token'] = 'auth-token';
      const userApi = await createFreshApiClient();

      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 401,
        json: () => Promise.resolve({ error: 'Unauthorized' }),
      });

      await expect(userApi.deleteAllNotes('wrong')).rejects.toMatchObject({ status: 401 });
      expect(sessionStore['user_session_token']).toBe('auth-token');
    });

    it('still treats a 401 with no body as an expired session', async () => {
      sessionStore['user_session_token'] = 'auth-token';
      const userApi = await createFreshApiClient();

      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 401,
        json: () => Promise.reject(new Error('no body')),
      });

      await expect(userApi.deleteAllNotes('whatever')).rejects.toMatchObject({ status: 401 });
      expect(sessionStore['user_session_token']).toBeUndefined();
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
