import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { passwordStorageService } from './passwordStorageService';
import { encodeStoredPassword } from '../utils/storedPassword';

const SHARED_KEY = 'jottery_stored_password';

describe('passwordStorageService', () => {
  let entries: Map<string, string>;

  /** Notebooks are told apart by the path they're opened at */
  const inNotebook = (path: string) => window.history.pushState({}, '', path);

  beforeEach(() => {
    entries = new Map();
    // Node's own global is undefined without a storage file, and shadows happy-dom's
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => entries.get(key) ?? null,
      setItem: (key: string, value: string) => void entries.set(key, value),
      removeItem: (key: string) => void entries.delete(key),
    });
    inNotebook('/');
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    inNotebook('/');
  });

  it('keeps each notebook separate', async () => {
    inNotebook('/recipes');
    await passwordStorageService.store('for recipes');

    inNotebook('/');
    expect(await passwordStorageService.get()).toBeNull();

    await passwordStorageService.store('for main');
    await passwordStorageService.clear();

    inNotebook('/recipes');
    expect(await passwordStorageService.get()).toBe('for recipes');
  });

  it('still reads a password saved by an older version', async () => {
    entries.set(SHARED_KEY, encodeStoredPassword('from before'));

    expect(await passwordStorageService.get()).toBe('from before');
  });

  it('moves it to this notebook the next time it is stored', async () => {
    entries.set(SHARED_KEY, encodeStoredPassword('from before'));

    await passwordStorageService.store('new');

    expect(entries.has(SHARED_KEY)).toBe(false);
    expect(await passwordStorageService.get()).toBe('new');
  });

  it('clears the old shared entry too', async () => {
    entries.set(SHARED_KEY, encodeStoredPassword('from before'));

    await passwordStorageService.clear();

    expect(await passwordStorageService.hasStored()).toBe(false);
  });
});
