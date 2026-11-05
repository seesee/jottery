/**
 * Admin API Client Tests
 *
 * Tests for the admin dashboard API client including authentication,
 * user management, device management, and error handling.
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

// The session lives in sessionStorage. localStorage is shared with the notes app
// on the same origin, so it's mocked separately to prove nothing goes there.
let store: Record<string, string> = {};
const sessionStorageMock = {
  getItem: vi.fn((key: string) => store[key] || null),
  setItem: vi.fn((key: string, value: string) => {
    store[key] = value;
  }),
  removeItem: vi.fn((key: string) => {
    delete store[key];
  }),
  clear: vi.fn(() => {
    store = {};
  }),
};

let sharedStore: Record<string, string> = {};
const localStorageMock = {
  getItem: vi.fn((key: string) => sharedStore[key] || null),
  setItem: vi.fn((key: string, value: string) => {
    sharedStore[key] = value;
  }),
  removeItem: vi.fn((key: string) => {
    delete sharedStore[key];
  }),
  clear: vi.fn(() => {
    sharedStore = {};
  }),
};

Object.defineProperty(globalThis, 'sessionStorage', {
  value: sessionStorageMock,
  writable: true,
});

Object.defineProperty(globalThis, 'localStorage', {
  value: localStorageMock,
  writable: true,
});

// Helper to get a fresh API client instance
async function getApi() {
  vi.resetModules();
  const module = await import('./api');
  return { api: module.api, ApiError: module.ApiError };
}

describe('ApiClient', () => {
  beforeEach(() => {
    store = {};
    sharedStore = {};
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('Authentication', () => {
    it('should login successfully and store session token', async () => {
      const { api } = await getApi();
      const mockResponse = {
        sessionId: 'test-session-123',
        expiresAt: '2025-01-01T00:00:00Z',
        user: { id: 'user-1', email: 'admin@test.com', isAdmin: true },
      };

      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
        new Response(JSON.stringify(mockResponse), { status: 200 })
      );

      const result = await api.login('admin@test.com', 'password123');

      expect(result).toEqual(mockResponse);
      expect(sessionStorageMock.setItem).toHaveBeenCalledWith(
        'admin_session_token',
        'test-session-123'
      );
      expect(api.isAuthenticated()).toBe(true);
    });

    it('never writes the session token to localStorage', async () => {
      const { api } = await getApi();
      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            sessionId: 'test-session-123',
            expiresAt: '2025-01-01T00:00:00Z',
            user: { id: 'user-1', email: 'admin@test.com', isAdmin: true },
          }),
          { status: 200 }
        )
      );

      await api.login('admin@test.com', 'password123');

      expect(localStorageMock.setItem).not.toHaveBeenCalled();
    });

    it('removes a token left in localStorage by an earlier version and ignores it', async () => {
      sharedStore['admin_session_token'] = 'stale-token';

      const { api } = await getApi();

      expect(sharedStore['admin_session_token']).toBeUndefined();
      expect(api.isAuthenticated()).toBe(false);
    });

    it('asks for a cookie session when logging in', async () => {
      const { api } = await getApi();
      const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            sessionId: 'test-session-123',
            expiresAt: '2025-01-01T00:00:00Z',
            user: { id: 'user-1', email: 'admin@test.com', isAdmin: true },
          }),
          { status: 200 }
        )
      );

      await api.login('admin@test.com', 'password123');

      const body = JSON.parse(fetchSpy.mock.calls[0][1]?.body as string);
      expect(body).toEqual({
        email: 'admin@test.com',
        password: 'password123',
        cookieSession: true,
      });
    });

    it('should throw ApiError on login failure', async () => {
      const { api, ApiError } = await getApi();
      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
        new Response(JSON.stringify({ error: 'Invalid credentials' }), { status: 401 })
      );

      await expect(api.login('wrong@test.com', 'wrong')).rejects.toThrow(ApiError);
    });

    it('should end the session on the server, then clear it locally', async () => {
      store['admin_session_token'] = 'test-session';
      const { api } = await getApi();
      const fetchSpy = vi
        .spyOn(globalThis, 'fetch')
        .mockResolvedValueOnce(new Response(null, { status: 204 }));

      await api.logout();

      expect(fetchSpy).toHaveBeenCalledWith(
        expect.stringContaining('/api/v1/user/logout'),
        expect.objectContaining({
          method: 'POST',
          headers: expect.objectContaining({ Authorization: 'Bearer test-session' }),
        })
      );
      expect(sessionStorageMock.removeItem).toHaveBeenCalledWith('admin_session_token');
      expect(api.isAuthenticated()).toBe(false);
    });

    it('should clear the session locally even if the server can\'t be reached', async () => {
      store['admin_session_token'] = 'test-session';
      const { api } = await getApi();
      vi.spyOn(console, 'warn').mockImplementation(() => {});
      vi.spyOn(globalThis, 'fetch').mockRejectedValueOnce(new Error('offline'));

      await api.logout();

      expect(store['admin_session_token']).toBeUndefined();
      expect(api.isAuthenticated()).toBe(false);
    });

    it('should include Authorization header when authenticated', async () => {
      store['admin_session_token'] = 'test-token';
      const { api } = await getApi();

      const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
        new Response(JSON.stringify({ users: [], total: 0 }), { status: 200 })
      );

      await api.listUsers();

      expect(fetchSpy).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({
          headers: expect.objectContaining({
            Authorization: 'Bearer test-token',
          }),
        })
      );
    });
  });

  describe('Cookie sessions', () => {
    const cookieLogin = {
      cookieSession: true,
      expiresAt: '2025-01-01T00:00:00Z',
      user: { id: 'user-1', email: 'admin@test.com', isAdmin: true },
    };

    it('should keep no token, and stay signed in', async () => {
      const { api } = await getApi();
      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
        new Response(JSON.stringify(cookieLogin), { status: 200 })
      );

      await api.login('admin@test.com', 'password123');

      expect(api.isAuthenticated()).toBe(true);
      expect(localStorageMock.setItem).not.toHaveBeenCalled();
    });

    it('should ignore a token sent alongside a cookie session', async () => {
      const { api } = await getApi();
      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
        new Response(JSON.stringify({ ...cookieLogin, sessionId: 'also-a-token' }), { status: 200 })
      );

      await api.login('admin@test.com', 'password123');

      expect(JSON.stringify(store)).not.toContain('also-a-token');
    });

    it('should send the CSRF header and no Authorization header', async () => {
      const { api } = await getApi();
      const fetchSpy = vi
        .spyOn(globalThis, 'fetch')
        .mockResolvedValueOnce(new Response(JSON.stringify(cookieLogin), { status: 200 }))
        .mockResolvedValueOnce(new Response(JSON.stringify({ users: [], total: 0 }), { status: 200 }));

      await api.login('admin@test.com', 'password123');
      await api.listUsers();

      const headers = fetchSpy.mock.calls[1][1]?.headers as Record<string, string>;
      expect(headers['Authorization']).toBeUndefined();
      expect(headers['X-Requested-With']).toBe('JotteryDashboard');
      expect(fetchSpy.mock.calls[1][1]?.credentials).toBe('include');
    });

    it('should not send the CSRF header with a bearer token', async () => {
      store['admin_session_token'] = 'test-token';
      const { api } = await getApi();
      const fetchSpy = vi
        .spyOn(globalThis, 'fetch')
        .mockResolvedValueOnce(new Response(JSON.stringify({ users: [], total: 0 }), { status: 200 }));

      await api.listUsers();

      const headers = fetchSpy.mock.calls[0][1]?.headers as Record<string, string>;
      expect(headers['X-Requested-With']).toBeUndefined();
    });

    it('should survive a reload of the same tab', async () => {
      const first = await getApi();
      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
        new Response(JSON.stringify(cookieLogin), { status: 200 })
      );
      await first.api.login('admin@test.com', 'password123');

      const reloaded = await getApi();

      expect(reloaded.api.isAuthenticated()).toBe(true);
    });

    it('should log out through the admin route, which receives the cookie', async () => {
      const { api } = await getApi();
      const fetchSpy = vi
        .spyOn(globalThis, 'fetch')
        .mockResolvedValueOnce(new Response(JSON.stringify(cookieLogin), { status: 200 }))
        .mockResolvedValueOnce(new Response(null, { status: 204 }));

      await api.login('admin@test.com', 'password123');
      await api.logout();

      expect(fetchSpy.mock.calls[1][0]).toContain('/api/v1/admin/logout');
      expect(api.isAuthenticated()).toBe(false);
    });
  });

  describe('User Management', () => {
    it('should list users', async () => {
      store['admin_session_token'] = 'test-token';
      const { api } = await getApi();

      const mockUsers = {
        users: [
          {
            id: 'user-1',
            email: 'user1@test.com',
            approved: true,
            isAdmin: false,
            isActive: true,
            createdAt: '2025-01-01T00:00:00Z',
            deviceCount: 2,
            noteCount: 10,
          },
        ],
        total: 1,
      };

      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
        new Response(JSON.stringify(mockUsers), { status: 200 })
      );

      const result = await api.listUsers();
      expect(result).toEqual(mockUsers);
    });

    it('should get user details', async () => {
      store['admin_session_token'] = 'test-token';
      const { api } = await getApi();

      const mockUser = {
        id: 'user-1',
        email: 'user1@test.com',
        approved: true,
        isAdmin: false,
        isActive: true,
        createdAt: '2025-01-01T00:00:00Z',
        approvedAt: '2025-01-02T00:00:00Z',
        lastLoginAt: null,
        storageQuotaMb: 100,
        stats: {
          devices: { total: 2, active: 1, lastSeen: null },
          notes: { count: 10 },
          attachments: { count: 5, totalBytes: 1024000 },
          lastSyncAt: null,
        },
        devices: [],
      };

      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
        new Response(JSON.stringify(mockUser), { status: 200 })
      );

      const result = await api.getUser('user-1');
      expect(result).toEqual(mockUser);
    });

    it('should approve user', async () => {
      store['admin_session_token'] = 'test-token';
      const { api } = await getApi();

      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(new Response(null, { status: 204 }));

      await expect(api.approveUser('user-1')).resolves.toBeUndefined();
    });

    it('should deactivate user', async () => {
      store['admin_session_token'] = 'test-token';
      const { api } = await getApi();

      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(new Response(null, { status: 204 }));

      await expect(api.deactivateUser('user-1')).resolves.toBeUndefined();
    });

    it('should activate user', async () => {
      store['admin_session_token'] = 'test-token';
      const { api } = await getApi();

      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(new Response(null, { status: 204 }));

      await expect(api.activateUser('user-1')).resolves.toBeUndefined();
    });

    it('should delete user', async () => {
      store['admin_session_token'] = 'test-token';
      const { api } = await getApi();

      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(new Response(null, { status: 204 }));

      await expect(api.deleteUser('user-1')).resolves.toBeUndefined();
    });

    it('should toggle admin status', async () => {
      store['admin_session_token'] = 'test-token';
      const { api } = await getApi();

      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(new Response(null, { status: 204 }));

      await expect(api.toggleAdmin('user-1', true)).resolves.toBeUndefined();
    });
  });

  describe('Device Management', () => {
    it('should get user devices', async () => {
      store['admin_session_token'] = 'test-token';
      const { api } = await getApi();

      const mockDevices = {
        devices: [
          {
            id: 'device-1',
            name: 'My Laptop',
            type: 'tui',
            createdAt: '2025-01-01T00:00:00Z',
            lastSeenAt: '2025-01-10T00:00:00Z',
            isActive: true,
          },
        ],
      };

      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
        new Response(JSON.stringify(mockDevices), { status: 200 })
      );

      const result = await api.getUserDevices('user-1');
      expect(result).toEqual(mockDevices);
    });

    it('should revoke device', async () => {
      store['admin_session_token'] = 'test-token';
      const { api } = await getApi();

      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(new Response(null, { status: 204 }));

      await expect(api.revokeDevice('device-1')).resolves.toBeUndefined();
    });

    it('should rename device', async () => {
      store['admin_session_token'] = 'test-token';
      const { api } = await getApi();

      const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
        new Response(null, { status: 204 })
      );

      await api.renameDevice('device-1', 'New Device Name');

      expect(fetchSpy).toHaveBeenCalledWith(
        expect.stringContaining('/api/v1/admin/devices/device-1'),
        expect.objectContaining({
          method: 'PATCH',
          body: JSON.stringify({ name: 'New Device Name' }),
        })
      );
    });
  });

  describe('Statistics', () => {
    it('should get dashboard stats', async () => {
      store['admin_session_token'] = 'test-token';
      const { api } = await getApi();

      const mockStats = {
        users: { total: 100, approved: 90, pending: 10, active: 85 },
        devices: { total: 200, active: 150 },
        notes: { total: 5000 },
        storage: { totalBytes: 1024000000, quotaBytes: 10240000000 },
      };

      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
        new Response(JSON.stringify(mockStats), { status: 200 })
      );

      const result = await api.getStats();
      expect(result).toEqual(mockStats);
    });
  });

  describe('Password Change', () => {
    it('should change password successfully', async () => {
      store['admin_session_token'] = 'test-token';
      const { api } = await getApi();

      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(new Response(null, { status: 204 }));

      await expect(api.changePassword('oldpass', 'newpass123')).resolves.toBeUndefined();
    });

    it('should throw on wrong current password', async () => {
      store['admin_session_token'] = 'test-token';
      const { api, ApiError } = await getApi();

      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
        new Response(JSON.stringify({ error: 'Invalid current password' }), { status: 401 })
      );

      await expect(api.changePassword('wrong', 'newpass123')).rejects.toThrow(ApiError);
    });
  });

  describe('Error Handling', () => {
    it('should handle network errors', async () => {
      store['admin_session_token'] = 'test-token';
      const { api } = await getApi();

      vi.spyOn(globalThis, 'fetch').mockRejectedValueOnce(new Error('Network error'));

      await expect(api.listUsers()).rejects.toThrow('Network error');
    });

    it('should handle malformed JSON responses', async () => {
      store['admin_session_token'] = 'test-token';
      const { api } = await getApi();

      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
        new Response('not json', { status: 200 })
      );

      await expect(api.listUsers()).rejects.toThrow();
    });

    it('should handle 500 server errors', async () => {
      store['admin_session_token'] = 'test-token';
      const { api, ApiError } = await getApi();

      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
        new Response(JSON.stringify({ error: 'Internal server error' }), { status: 500 })
      );

      try {
        await api.listUsers();
        expect.fail('Should have thrown');
      } catch (e) {
        expect(e).toBeInstanceOf(ApiError);
        expect((e as InstanceType<typeof ApiError>).status).toBe(500);
      }
    });

    it('should handle 403 forbidden errors', async () => {
      store['admin_session_token'] = 'test-token';
      const { api, ApiError } = await getApi();

      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
        new Response(JSON.stringify({ error: 'Admin access required' }), { status: 403 })
      );

      try {
        await api.listUsers();
        expect.fail('Should have thrown');
      } catch (e) {
        expect(e).toBeInstanceOf(ApiError);
        expect((e as InstanceType<typeof ApiError>).status).toBe(403);
        expect((e as InstanceType<typeof ApiError>).message).toBe('Admin access required');
      }
    });

    it('should handle errors without JSON body', async () => {
      store['admin_session_token'] = 'test-token';
      const { api, ApiError } = await getApi();

      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
        new Response('Unauthorized', {
          status: 401,
          headers: { 'Content-Type': 'text/plain' },
        })
      );

      try {
        await api.listUsers();
        expect.fail('Should have thrown');
      } catch (e) {
        expect(e).toBeInstanceOf(ApiError);
        expect((e as InstanceType<typeof ApiError>).status).toBe(401);
      }
    });
  });
});

describe('ApiError', () => {
  it('should have correct properties', async () => {
    const { ApiError } = await getApi();
    const error = new ApiError(404, 'Not found');

    expect(error.status).toBe(404);
    expect(error.message).toBe('Not found');
    expect(error.name).toBe('ApiError');
    expect(error).toBeInstanceOf(Error);
  });
});
