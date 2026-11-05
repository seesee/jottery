import { describe, it, expect } from 'vitest';
import { ApiError } from './dashboardClient';
import { describeError, CHARACTER_CLASSES, KNOWN_MESSAGES } from './apiErrors';
import en from '../locales/en-GB.json';

// Shows which key was chosen, and with what, instead of a translation
const t = (key: string, options?: { values?: Record<string, string | number> }) =>
  options?.values ? `${key}:${JSON.stringify(options.values)}` : key;

describe('describeError', () => {
  it('translates server messages it knows', () => {
    const err = new ApiError(400, 'New password must be different from the current one');
    expect(describeError(err, t)).toBe('errors.passwordSameAsCurrent');
  });

  it('treats a 401 reaching a page as a wrong password, since expiry never does', () => {
    expect(describeError(new ApiError(401, 'Unauthorized'), t)).toBe('errors.wrongPassword');
  });

  it('says how long to wait when the server does', () => {
    const err = new ApiError(429, 'Too many attempts. Try again in 30 seconds.', 'rate_limited', 30);
    expect(describeError(err, t)).toBe('errors.rateLimited:{"seconds":30}');
  });

  it('still explains a 429 with no wait time', () => {
    expect(describeError(new ApiError(429, ''), t)).toBe('errors.tooManyAttempts');
  });

  it('keeps a server message it has no translation for', () => {
    expect(describeError(new ApiError(400, 'A message added to a newer server'), t)).toBe(
      'A message added to a newer server',
    );
  });

  it('translates the password rules, whose message varies with what is missing', () => {
    const err = new ApiError(400, 'Password requires at least 3 of 4 character types. Missing: uppercase letter, number');
    expect(describeError(err, t)).toBe(
      'errors.passwordComplexity:{"required":3,"missing":"errors.passwordClass.uppercase, errors.passwordClass.number"}',
    );
  });

  it('keeps a character type it has no name for, rather than dropping it', () => {
    const err = new ApiError(400, 'Password requires at least 4 of 4 character types. Missing: emoji');
    expect(describeError(err, t)).toBe('errors.passwordComplexity:{"required":4,"missing":"emoji"}');
  });

  it('has a real translation for every character type', () => {
    for (const key of Object.values(CHARACTER_CLASSES)) {
      const value = key.split('.').reduce<unknown>((node, part) => (node as Record<string, unknown>)?.[part], en);
      expect(typeof value, key).toBe('string');
    }
  });

  it('only maps server messages to keys that exist', () => {
    for (const key of Object.values(KNOWN_MESSAGES)) {
      const value = key.split('.').reduce<unknown>((node, part) => (node as Record<string, unknown>)?.[part], en);
      expect(typeof value, key).toBe('string');
    }
  });

  it('explains a 403 with no body, which the middleware sends', () => {
    expect(describeError(new ApiError(403, ''), t)).toBe('errors.forbidden');
  });

  it('never returns nothing for an empty response', () => {
    expect(describeError(new ApiError(500, ''), t)).toBe('errors.unknown');
  });

  it('prefers the page\'s own fallback over the generic one', () => {
    expect(describeError(new ApiError(500, ''), t, 'users.failedToLoad')).toBe('users.failedToLoad');
  });

  it('reports a failed fetch as a connection problem', () => {
    expect(describeError(new TypeError('Failed to fetch'), t)).toBe('errors.network');
  });

  it('does not show the text of anything else that was thrown', () => {
    expect(describeError(new Error('internal detail'), t)).toBe('errors.unknown');
    expect(describeError('a string', t, 'dashboard.failedToLoad')).toBe('dashboard.failedToLoad');
  });
});
