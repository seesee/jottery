import { describe, it, expect, vi } from 'vitest';
import { DashboardAuth, type AuthBackend } from './dashboardAuth.svelte';

const user = { id: 'u1', email: 'admin@example.com', isAdmin: true };

/** A backend that remembers its listeners, so a test can act as the server refusing. */
function fakeBackend(overrides: Partial<AuthBackend> = {}) {
  let expire = () => {};
  let requireChange = () => {};
  const backend: AuthBackend = {
    isAuthenticated: () => false,
    login: vi.fn(async () => ({ user })),
    logout: vi.fn(async () => {}),
    changePassword: vi.fn(async () => {}),
    currentUser: vi.fn(async () => ({ email: user.email })),
    onSessionExpired: (listener) => {
      expire = listener;
    },
    onPasswordChangeRequired: (listener) => {
      requireChange = listener;
    },
    ...overrides,
  };
  return { backend, expire: () => expire(), requireChange: () => requireChange() };
}

describe('DashboardAuth', () => {
  describe('signing in', () => {
    it('lets a normal account straight in', async () => {
      const auth = new DashboardAuth(fakeBackend().backend);

      await auth.login('admin@example.com', 'a-password');

      expect(auth.isAuthenticated).toBe(true);
      expect(auth.mustChangePassword).toBe(false);
      expect(auth.user).toEqual(user);
    });

    it('asks for a new password when the server says so', async () => {
      const { backend } = fakeBackend({ login: vi.fn(async () => ({ user, mustChangePassword: true })) });
      const auth = new DashboardAuth(backend);

      await auth.login('admin@example.com', 'a-password');

      expect(auth.isAuthenticated).toBe(true);
      expect(auth.mustChangePassword).toBe(true);
    });

    it('treats an older server, which never sends the field, as not requiring it', async () => {
      const auth = new DashboardAuth(fakeBackend().backend);

      await auth.login('admin@example.com', 'a-password');

      expect(auth.mustChangePassword).toBe(false);
    });

    it('stays signed out when login fails', async () => {
      const { backend } = fakeBackend({ login: vi.fn().mockRejectedValue(new Error('no')) });
      const auth = new DashboardAuth(backend);

      await expect(auth.login('a', 'b')).rejects.toThrow('no');

      expect(auth.isAuthenticated).toBe(false);
    });
  });

  describe('the server requiring a new password later', () => {
    it('shows the screen for a session already open when the rule starts applying', () => {
      const fake = fakeBackend({ isAuthenticated: () => true });
      const auth = new DashboardAuth(fake.backend);

      fake.requireChange();

      expect(auth.mustChangePassword).toBe(true);
      expect(auth.isAuthenticated).toBe(true);
    });

    it('survives a reload, because restoring the session gets the same refusal', async () => {
      const fake = fakeBackend({ isAuthenticated: () => true });
      fake.backend.currentUser = vi.fn(async () => {
        fake.requireChange();
        throw new Error('403');
      });
      const auth = new DashboardAuth(fake.backend);

      await auth.restore();

      expect(auth.mustChangePassword).toBe(true);
      expect(auth.restoring).toBe(false);
    });
  });

  describe('changing the password', () => {
    it('opens the dashboard afterwards', async () => {
      const { backend } = fakeBackend({ login: vi.fn(async () => ({ user, mustChangePassword: true })) });
      const auth = new DashboardAuth(backend);
      await auth.login('admin@example.com', 'old-password');

      await auth.changePassword('old-password', 'a-new-long-password');

      expect(backend.changePassword).toHaveBeenCalledWith('old-password', 'a-new-long-password');
      expect(auth.mustChangePassword).toBe(false);
    });

    it('fetches who is signed in if a reload lost it', async () => {
      const fake = fakeBackend({ isAuthenticated: () => true });
      const auth = new DashboardAuth(fake.backend);
      fake.requireChange();

      await auth.changePassword('old-password', 'a-new-long-password');

      expect(auth.user).toEqual({ email: user.email });
    });

    it('keeps asking when the change fails', async () => {
      const { backend } = fakeBackend({
        login: vi.fn(async () => ({ user, mustChangePassword: true })),
        changePassword: vi.fn().mockRejectedValue(new Error('wrong password')),
      });
      const auth = new DashboardAuth(backend);
      await auth.login('admin@example.com', 'old-password');

      await expect(auth.changePassword('bad', 'a-new-long-password')).rejects.toThrow('wrong password');

      expect(auth.mustChangePassword).toBe(true);
      expect(auth.isAuthenticated).toBe(true);
    });
  });

  describe('restoring a session after a reload', () => {
    it('restores who is signed in', async () => {
      const auth = new DashboardAuth(fakeBackend({ isAuthenticated: () => true }).backend);
      expect(auth.restoring).toBe(true);

      await auth.restore();

      expect(auth.user).toEqual({ email: user.email });
      expect(auth.restoring).toBe(false);
      expect(auth.isAuthenticated).toBe(true);
    });

    it('asks nothing of the server without a session', async () => {
      const { backend } = fakeBackend();
      const auth = new DashboardAuth(backend);

      await auth.restore();

      expect(backend.currentUser).not.toHaveBeenCalled();
      expect(auth.restoring).toBe(false);
    });

    it('keeps the session when the server can\'t be reached', async () => {
      const { backend } = fakeBackend({
        isAuthenticated: () => true,
        currentUser: vi.fn().mockRejectedValue(new TypeError('Failed to fetch')),
      });
      const auth = new DashboardAuth(backend);

      await auth.restore();

      expect(auth.isAuthenticated).toBe(true);
      expect(auth.restoring).toBe(false);
    });
  });

  describe('an expired session', () => {
    it('signs out and says why', () => {
      const fake = fakeBackend({ isAuthenticated: () => true });
      const auth = new DashboardAuth(fake.backend);

      fake.expire();

      expect(auth.isAuthenticated).toBe(false);
      expect(auth.sessionExpired).toBe(true);
    });

    it('forgets a required password change, which the next sign-in reports afresh', () => {
      const fake = fakeBackend({ isAuthenticated: () => true });
      const auth = new DashboardAuth(fake.backend);
      fake.requireChange();

      fake.expire();

      expect(auth.mustChangePassword).toBe(false);
    });

    it('clears the message once signed in again', async () => {
      const fake = fakeBackend({ isAuthenticated: () => true });
      const auth = new DashboardAuth(fake.backend);
      fake.expire();

      await auth.login('admin@example.com', 'a-password');

      expect(auth.sessionExpired).toBe(false);
    });
  });

  describe('signing out', () => {
    it('does not call a deliberate sign-out an expired session', async () => {
      const fake = fakeBackend({ isAuthenticated: () => true });
      // A stale token makes the logout request itself report expiry
      fake.backend.logout = vi.fn(async () => fake.expire());
      const auth = new DashboardAuth(fake.backend);

      await auth.logout();

      expect(auth.isAuthenticated).toBe(false);
      expect(auth.sessionExpired).toBe(false);
    });

    it('works from the change-password screen', async () => {
      const { backend } = fakeBackend({ login: vi.fn(async () => ({ user, mustChangePassword: true })) });
      const auth = new DashboardAuth(backend);
      await auth.login('admin@example.com', 'a-password');

      await auth.logout();

      expect(auth.isAuthenticated).toBe(false);
      expect(auth.mustChangePassword).toBe(false);
    });
  });
});
