/**
 * Persist Session stores the password so a refresh doesn't ask for it. The app
 * supports languages whose passwords are outside Latin1, which btoa can't encode.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { get } from 'svelte/store';
import {
  clearSessionPassword,
  getSessionPassword,
  hasStoredSession,
  hasValidSession,
  storeSessionPassword,
} from './sessionStorageService';
import { clearStoredPassword, getStoredPassword, storePassword } from './passwordStorageService';
import { isPersistSessionExpired } from './autoLockService';
import { settings } from '../stores/appStore';

/** Node's own localStorage global hides happy-dom's, so give these tests one. */
function memoryStorage(): Storage {
  const values = new Map<string, string>();
  return {
    get length() { return values.size; },
    clear: () => values.clear(),
    getItem: (key: string) => values.get(key) ?? null,
    key: (index: number) => Array.from(values.keys())[index] ?? null,
    removeItem: (key: string) => { values.delete(key); },
    setItem: (key: string, value: string) => { values.set(key, value); },
  };
}

const PASSWORDS = ['пароль-длинный', '日本語パスワード', 'κωδικός πρόσβασης', 'zażółć gęślą jaźń', 'şifre-ığüöç', 'emoji-🔑-key', 'pässwörd-latin1', 'plain-ascii'];

describe('Persist Session', () => {
  beforeEach(() => {
    sessionStorage.clear();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    settings.update(s => ({ ...s, persistSession: false }));
  });

  for (const password of PASSWORDS) {
    it(`round-trips ${password}`, () => {
      expect(storeSessionPassword(password)).toBe(true);
      expect(getSessionPassword(30)).toBe(password);
    });

    it(`keeps a session valid for ${password}`, () => {
      storeSessionPassword(password);
      expect(hasValidSession(30)).toBe(true);
    });
  }

  it('still reads a value an older version stored', () => {
    sessionStorage.setItem('jottery_session_password', btoa('pässwörd-latin1'));
    sessionStorage.setItem('jottery_session_timestamp', Date.now().toString());
    expect(getSessionPassword(30)).toBe('pässwörd-latin1');
  });

  it('reports failure when the password cannot be stored', () => {
    const storage = memoryStorage();
    const setItem = storage.setItem;
    storage.setItem = (key, value) => {
      if (key === 'jottery_session_password') throw new Error('full');
      setItem(key, value);
    };
    vi.stubGlobal('sessionStorage', storage);

    expect(storeSessionPassword('anything')).toBe(false);

    vi.unstubAllGlobals();
  });

  describe('auto-lock', () => {
    it('does not lock anyone out when nothing was ever stored', () => {
      settings.update(s => ({ ...s, persistSession: true, persistSessionTimeout: 30 }));
      expect(hasStoredSession()).toBe(false);
      expect(isPersistSessionExpired()).toBe(false);
    });

    it('does not lock a valid session', () => {
      settings.update(s => ({ ...s, persistSession: true, persistSessionTimeout: 30 }));
      storeSessionPassword('пароль-длинный');
      expect(isPersistSessionExpired()).toBe(false);
    });

    it('still locks a session that has expired', () => {
      settings.update(s => ({ ...s, persistSession: true, persistSessionTimeout: 30 }));
      storeSessionPassword('пароль-длинный');
      sessionStorage.setItem('jottery_session_timestamp', (Date.now() - 31 * 60 * 1000).toString());
      expect(isPersistSessionExpired()).toBe(true);
    });

    it('is off when the setting is off', () => {
      expect(get(settings).persistSession).toBeFalsy();
      expect(isPersistSessionExpired()).toBe(false);
    });

    it('treats a cleared session as never stored', () => {
      storeSessionPassword('x');
      clearSessionPassword();
      expect(hasStoredSession()).toBe(false);
    });
  });
});

describe('Remember Password', () => {
  beforeEach(() => vi.stubGlobal('localStorage', memoryStorage()));
  afterEach(() => vi.unstubAllGlobals());

  for (const password of PASSWORDS) {
    it(`round-trips ${password}`, async () => {
      await storePassword(password);
      expect(await getStoredPassword()).toBe(password);
    });
  }

  it('still reads a value an older version stored', async () => {
    localStorage.setItem('jottery_stored_password', btoa('pässwörd-latin1'));
    expect(await getStoredPassword()).toBe('pässwörd-latin1');
  });

  it('clears a value that cannot be decoded', async () => {
    localStorage.setItem('jottery_stored_password', 'u8:%%%not-base64');
    expect(await getStoredPassword()).toBeNull();
    expect(localStorage.getItem('jottery_stored_password')).toBeNull();
    await clearStoredPassword();
  });
});
