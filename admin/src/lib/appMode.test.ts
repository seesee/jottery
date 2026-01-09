// Tests for app mode detection
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { getAppMode } from './appMode';

describe('appMode', () => {
  beforeEach(() => {
    // Reset location mock before each test
    vi.stubGlobal('location', { pathname: '/' });
  });

  describe('getAppMode', () => {
    it('returns "admin" for /admin path', () => {
      vi.stubGlobal('location', { pathname: '/admin' });
      expect(getAppMode()).toBe('admin');
    });

    it('returns "admin" for /admin/users path', () => {
      vi.stubGlobal('location', { pathname: '/admin/users' });
      expect(getAppMode()).toBe('admin');
    });

    it('returns "admin" for root path', () => {
      vi.stubGlobal('location', { pathname: '/' });
      expect(getAppMode()).toBe('admin');
    });

    it('returns "user" for /user path', () => {
      vi.stubGlobal('location', { pathname: '/user' });
      expect(getAppMode()).toBe('user');
    });

    it('returns "user" for /user/settings path', () => {
      vi.stubGlobal('location', { pathname: '/user/settings' });
      expect(getAppMode()).toBe('user');
    });

    it('returns "admin" for paths not starting with /user', () => {
      vi.stubGlobal('location', { pathname: '/something-else' });
      expect(getAppMode()).toBe('admin');
    });

    // Note: /users starts with /user, so it returns 'user' mode
    // This is intentional as the user portal paths all start with /user
    it('returns "user" for /users path (starts with /user)', () => {
      vi.stubGlobal('location', { pathname: '/users' });
      expect(getAppMode()).toBe('user');
    });
  });
});
