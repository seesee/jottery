import { describe, it, expect } from 'vitest';
import { keyManager } from './keyManager';

describe('clearMasterKey', () => {
  it('overwrites the raw key bytes and what else it held', () => {
    const keyBytes = crypto.getRandomValues(new Uint8Array(32));
    const masterKey = { key: {} as CryptoKey, keyBytes, derivedAt: Date.now(), password: 'secret' };
    keyManager.setMasterKey(masterKey);

    keyManager.clearMasterKey();

    expect(Array.from(keyBytes).every(byte => byte === 0)).toBe(true);
    expect(masterKey.password).toBeUndefined();
    expect(masterKey.derivedAt).toBe(0);
    expect(keyManager.getMasterKey()).toBeNull();
  });
});
