import { describe, it, expect, beforeEach, vi } from 'vitest';
import { ApiError, DashboardClient } from './dashboardClient';

function memoryStorage() {
  let items: Record<string, string> = {};
  return {
    getItem: (key: string) => items[key] ?? null,
    setItem: (key: string, value: string) => {
      items[key] = value;
    },
    removeItem: (key: string) => {
      delete items[key];
    },
    clear: () => {
      items = {};
    },
  };
}

vi.stubGlobal('sessionStorage', memoryStorage());
vi.stubGlobal('localStorage', memoryStorage());

const mockFetch = vi.fn();
vi.stubGlobal('fetch', mockFetch);

/** What the session middleware sends: a status and no body. */
function emptyResponse(status: number) {
  return { ok: false, status, json: () => Promise.reject(new SyntaxError('Unexpected end of JSON input')) };
}

/** What a route handler sends through AppError. */
function jsonResponse(status: number, body: unknown) {
  return { ok: status < 400, status, json: () => Promise.resolve(body) };
}

function signedInClient() {
  const client = new DashboardClient('test_token');
  client.session.start({ sessionId: 'a-token' });
  return client;
}

describe('DashboardClient', () => {
  beforeEach(() => {
    mockFetch.mockReset();
    sessionStorage.clear();
    localStorage.clear();
  });

  describe('an expired session', () => {
    it('clears the session and tells the app when the server rejects the token', async () => {
      const client = signedInClient();
      const expired = vi.fn();
      client.onSessionExpired(expired);
      mockFetch.mockResolvedValue(emptyResponse(401));

      await expect(client.request('GET', '/api/v1/admin/users')).rejects.toThrow(ApiError);

      expect(expired).toHaveBeenCalledOnce();
      expect(client.session.isActive).toBe(false);
      expect(sessionStorage.getItem('test_token')).toBeNull();
    });

    it('reports it once when several requests fail together', async () => {
      const client = signedInClient();
      const expired = vi.fn();
      client.onSessionExpired(expired);
      mockFetch.mockResolvedValue(emptyResponse(401));

      await Promise.allSettled([client.request('GET', '/a'), client.request('GET', '/b')]);

      expect(expired).toHaveBeenCalledOnce();
    });

    it('does not report expiry without a session, as after a failed login', async () => {
      const client = new DashboardClient('test_token');
      const expired = vi.fn();
      client.onSessionExpired(expired);
      mockFetch.mockResolvedValue(emptyResponse(401));

      await expect(client.request('POST', '/api/v1/auth/login')).rejects.toThrow(ApiError);

      expect(expired).not.toHaveBeenCalled();
    });
  });

  describe('a route that checks a password', () => {
    it('keeps the session when the password was wrong', async () => {
      const client = signedInClient();
      const expired = vi.fn();
      client.onSessionExpired(expired);
      mockFetch.mockResolvedValue(jsonResponse(401, { error: 'Unauthorized' }));

      await expect(
        client.request('POST', '/api/v1/admin/change-password', {}, { checksPassword: true }),
      ).rejects.toMatchObject({ status: 401 });

      expect(expired).not.toHaveBeenCalled();
      expect(client.session.isActive).toBe(true);
    });

    it('still treats the middleware rejecting the session as expiry', async () => {
      const client = signedInClient();
      const expired = vi.fn();
      client.onSessionExpired(expired);
      mockFetch.mockResolvedValue(emptyResponse(401));

      await expect(
        client.request('POST', '/api/v1/admin/change-password', {}, { checksPassword: true }),
      ).rejects.toThrow(ApiError);

      expect(expired).toHaveBeenCalledOnce();
    });

    it('treats a 401 with a body as expiry on routes that check no password', async () => {
      const client = signedInClient();
      const expired = vi.fn();
      client.onSessionExpired(expired);
      mockFetch.mockResolvedValue(jsonResponse(401, { error: 'Unauthorized' }));

      await expect(client.request('GET', '/api/v1/admin/stats')).rejects.toThrow(ApiError);

      expect(expired).toHaveBeenCalledOnce();
    });
  });

  describe('a forbidden request', () => {
    it('keeps the session, because 403 is not expiry', async () => {
      const client = signedInClient();
      const expired = vi.fn();
      client.onSessionExpired(expired);
      mockFetch.mockResolvedValue(emptyResponse(403));

      await expect(client.request('GET', '/api/v1/admin/stats')).rejects.toMatchObject({ status: 403 });

      expect(expired).not.toHaveBeenCalled();
      expect(client.session.isActive).toBe(true);
    });

    it('reports a required password change', async () => {
      const client = signedInClient();
      const required = vi.fn();
      client.onPasswordChangeRequired(required);
      mockFetch.mockResolvedValue(
        jsonResponse(403, { error: 'You must change your password before continuing', code: 'password_change_required' }),
      );

      await expect(client.request('GET', '/api/v1/admin/stats')).rejects.toMatchObject({
        status: 403,
        code: 'password_change_required',
      });

      expect(required).toHaveBeenCalledOnce();
      expect(client.session.isActive).toBe(true);
    });

    it('does not report a password change for any other 403', async () => {
      const client = signedInClient();
      const required = vi.fn();
      client.onPasswordChangeRequired(required);
      mockFetch.mockResolvedValue(jsonResponse(403, { error: 'Admin access required' }));

      await expect(client.request('GET', '/api/v1/admin/stats')).rejects.toThrow(ApiError);

      expect(required).not.toHaveBeenCalled();
    });
  });

  describe('error details', () => {
    it('carries the code, message and wait time the server sent', async () => {
      const client = signedInClient();
      mockFetch.mockResolvedValue(
        jsonResponse(429, { error: 'Too many attempts. Try again in 30 seconds.', code: 'rate_limited', retryAfter: 30 }),
      );

      await expect(client.request('POST', '/api/v1/user/login')).rejects.toMatchObject({
        status: 429,
        code: 'rate_limited',
        retryAfter: 30,
        message: 'Too many attempts. Try again in 30 seconds.',
      });
    });

    it('has no English fallback of its own for an empty response', async () => {
      const client = signedInClient();
      mockFetch.mockResolvedValue(emptyResponse(500));

      await expect(client.request('GET', '/x')).rejects.toMatchObject({ status: 500, message: '' });
    });

    it('ignores a body that is not an object', async () => {
      const client = signedInClient();
      mockFetch.mockResolvedValue(jsonResponse(500, 'plain string'));

      await expect(client.request('GET', '/x')).rejects.toMatchObject({ status: 500, message: '' });
    });
  });

  it('sends the session and a JSON body', async () => {
    const client = signedInClient();
    mockFetch.mockResolvedValue({ ok: true, status: 200, json: () => Promise.resolve({ ok: 1 }) });

    await expect(client.request('POST', '/x', { a: 1 })).resolves.toEqual({ ok: 1 });

    expect(mockFetch).toHaveBeenCalledWith('/x', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: 'Bearer a-token' },
      body: '{"a":1}',
      credentials: 'include',
    });
  });

  it('returns nothing for 204', async () => {
    const client = signedInClient();
    mockFetch.mockResolvedValue({ ok: true, status: 204, json: () => Promise.reject(new Error('no body')) });

    await expect(client.request('POST', '/x')).resolves.toBeUndefined();
  });
});
