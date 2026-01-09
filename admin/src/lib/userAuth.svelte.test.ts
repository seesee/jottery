// Tests for user authentication store
// Note: Testing Svelte 5 runes requires special handling as $state is compile-time transformed
import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';

// Mock localStorage
const localStorageMock = {
  store: {} as Record<string, string>,
  getItem: vi.fn((key: string) => localStorageMock.store[key] || null),
  setItem: vi.fn((key: string, value: string) => {
    localStorageMock.store[key] = value;
  }),
  removeItem: vi.fn((key: string) => {
    delete localStorageMock.store[key];
  }),
  clear: vi.fn(() => {
    localStorageMock.store = {};
  }),
};
vi.stubGlobal('localStorage', localStorageMock);

// Mock fetch for userApi
const mockFetch = vi.fn();
vi.stubGlobal('fetch', mockFetch);

// Helper to get fresh auth store
async function createFreshAuthStore() {
  vi.resetModules();
  const module = await import('./userAuth.svelte');
  return module.userAuth;
}

describe('UserAuthStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorageMock.store = {};
  });

  afterEach(() => {
    vi.resetModules();
  });

  describe('initial state', () => {
    it('isAuthenticated is false when no token in localStorage', async () => {
      const userAuth = await createFreshAuthStore();

      expect(userAuth.isAuthenticated).toBe(false);
    });

    it('isAuthenticated is true when token exists in localStorage', async () => {
      localStorageMock.store['user_session_token'] = 'existing-token';
      const userAuth = await createFreshAuthStore();

      expect(userAuth.isAuthenticated).toBe(true);
    });

    it('user is null initially', async () => {
      const userAuth = await createFreshAuthStore();

      expect(userAuth.user).toBeNull();
    });
  });

  describe('login', () => {
    it('sets isAuthenticated to true after successful login', async () => {
      const userAuth = await createFreshAuthStore();

      mockFetch.mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: () =>
          Promise.resolve({
            sessionId: 'new-token',
            expiresAt: '2025-01-20T00:00:00Z',
            user: { id: '1', email: 'test@example.com', isAdmin: false },
          }),
      });

      await userAuth.login('test@example.com', 'password123');

      expect(userAuth.isAuthenticated).toBe(true);
    });

    it('sets user data after successful login', async () => {
      const userAuth = await createFreshAuthStore();

      mockFetch.mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: () =>
          Promise.resolve({
            sessionId: 'new-token',
            expiresAt: '2025-01-20T00:00:00Z',
            user: { id: 'user-123', email: 'test@example.com', isAdmin: false },
          }),
      });

      await userAuth.login('test@example.com', 'password123');

      expect(userAuth.user).toEqual({
        id: 'user-123',
        email: 'test@example.com',
        isAdmin: false,
      });
    });

    it('throws on failed login', async () => {
      const userAuth = await createFreshAuthStore();

      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 401,
        json: () => Promise.resolve({ error: 'Invalid credentials' }),
      });

      await expect(userAuth.login('test@example.com', 'wrong')).rejects.toThrow();
      expect(userAuth.isAuthenticated).toBe(false);
    });
  });

  describe('logout', () => {
    it('sets isAuthenticated to false', async () => {
      localStorageMock.store['user_session_token'] = 'existing-token';
      const userAuth = await createFreshAuthStore();

      // Login first to set user
      mockFetch.mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: () =>
          Promise.resolve({
            sessionId: 'new-token',
            expiresAt: '2025-01-20T00:00:00Z',
            user: { id: '1', email: 'test@example.com', isAdmin: false },
          }),
      });
      await userAuth.login('test@example.com', 'password');

      // Then logout
      mockFetch.mockResolvedValueOnce({
        ok: true,
        status: 204,
      });
      await userAuth.logout();

      expect(userAuth.isAuthenticated).toBe(false);
    });

    it('clears user data', async () => {
      const userAuth = await createFreshAuthStore();

      // Login first
      mockFetch.mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: () =>
          Promise.resolve({
            sessionId: 'new-token',
            expiresAt: '2025-01-20T00:00:00Z',
            user: { id: '1', email: 'test@example.com', isAdmin: false },
          }),
      });
      await userAuth.login('test@example.com', 'password');

      expect(userAuth.user).not.toBeNull();

      // Logout
      mockFetch.mockResolvedValueOnce({
        ok: true,
        status: 204,
      });
      await userAuth.logout();

      expect(userAuth.user).toBeNull();
    });
  });

  describe('clearAuth', () => {
    it('clears authentication state synchronously', async () => {
      const userAuth = await createFreshAuthStore();

      // Login first
      mockFetch.mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: () =>
          Promise.resolve({
            sessionId: 'new-token',
            expiresAt: '2025-01-20T00:00:00Z',
            user: { id: '1', email: 'test@example.com', isAdmin: false },
          }),
      });
      await userAuth.login('test@example.com', 'password');

      // Clear auth (used when account is deactivated/deleted)
      userAuth.clearAuth();

      expect(userAuth.isAuthenticated).toBe(false);
      expect(userAuth.user).toBeNull();
    });
  });
});
