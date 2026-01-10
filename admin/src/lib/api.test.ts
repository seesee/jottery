/**
 * Admin API Client Tests
 *
 * Tests for the admin dashboard API client including authentication,
 * user management, device management, and error handling.
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

// Mock localStorage
let store: Record<string, string> = {};
const localStorageMock = {
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
      expect(localStorageMock.setItem).toHaveBeenCalledWith(
        'admin_session_token',
        'test-session-123'
      );
      expect(api.isAuthenticated()).toBe(true);
    });

    it('should throw ApiError on login failure', async () => {
      const { api, ApiError } = await getApi();
      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
        new Response(JSON.stringify({ error: 'Invalid credentials' }), { status: 401 })
      );

      await expect(api.login('wrong@test.com', 'wrong')).rejects.toThrow(ApiError);
    });

    it('should clear session on logout', async () => {
      store['admin_session_token'] = 'test-session';
      const { api } = await getApi();

      api.logout();

      expect(localStorageMock.removeItem).toHaveBeenCalledWith('admin_session_token');
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
