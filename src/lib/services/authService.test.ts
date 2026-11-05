import { describe, it, expect, beforeAll, afterAll, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { setupServer } from 'msw/node';
import { AuthApiError, authService } from './authService';

const ENDPOINT = 'https://sync.example.com';
const URL = `${ENDPOINT}/api/v1/auth/wrapped-key`;
const KEY = { blob: 'b', kdfVersion: 2, kdfIterations: 1_000_000 };
const server = setupServer();

describe('wrapped key requests', () => {
  beforeAll(() => server.listen({ onUnhandledRequest: 'error' }));
  afterEach(() => server.resetHandlers());
  afterAll(() => server.close());

  /** Records what a PUT sent, and answers with this status */
  function capturePut(status = 200) {
    const seen: { body?: Record<string, unknown>; ifMatch?: string | null } = {};
    server.use(http.put(URL, async ({ request }) => {
      seen.body = await request.json() as Record<string, unknown>;
      seen.ifMatch = request.headers.get('If-Match');
      return new HttpResponse('{}', { status });
    }));
    return seen;
  }

  it('sends replace in the body, and no If-Match', async () => {
    const seen = capturePut();

    await authService.putWrappedKey(ENDPOINT, 'token', KEY, { replace: false });

    expect(seen.body?.replace).toBe(false);
    expect(seen.ifMatch).toBeNull();
  });

  it('sends If-Match as a header, and no replace', async () => {
    const seen = capturePut();

    await authService.putWrappedKey(ENDPOINT, 'token', KEY, { ifMatch: '2026-11-05T03:00:00Z' });

    expect(seen.ifMatch).toBe('2026-11-05T03:00:00Z');
    expect(seen.body).not.toHaveProperty('replace');
  });

  it('reports a refused condition as a 409 the caller can recognise', async () => {
    capturePut(409);

    const failure = await authService.putWrappedKey(ENDPOINT, 'token', KEY, { replace: false }).catch(error => error);

    expect(failure).toBeInstanceOf(AuthApiError);
    expect(failure.status).toBe(409);
  });

  it('passes updatedAt on, unchanged', async () => {
    server.use(http.get(URL, () => HttpResponse.json({ ...KEY, updatedAt: '2026-11-05T03:00:00Z' })));

    expect((await authService.getWrappedKey(ENDPOINT, 'token'))?.updatedAt).toBe('2026-11-05T03:00:00Z');
  });

  it.each([
    ['a line break, which fetch would throw on', 'x\r\nInjected: yes'],
    ['something that is not a string', 12],
    ['nothing', undefined],
    ['an oversized value', 'x'.repeat(5000)],
  ])('treats %s as a server with no conditional writes', async (_name, updatedAt) => {
    server.use(http.get(URL, () => HttpResponse.json({ ...KEY, updatedAt })));

    expect((await authService.getWrappedKey(ENDPOINT, 'token'))?.updatedAt).toBeUndefined();
  });

  it('returns null when the account has no key', async () => {
    server.use(http.get(URL, () => new HttpResponse('none', { status: 404 })));

    expect(await authService.getWrappedKey(ENDPOINT, 'token')).toBeNull();
  });
});
