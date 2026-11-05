/**
 * Key manager service for handling master key lifecycle.
 * autoLockService decides when to lock.
 */

import type { KeyManager, MasterKey } from '../types';

class KeyManagerService implements KeyManager {
  private masterKey: MasterKey | null = null;
  private lockCallbacks: Set<() => void> = new Set();

  /**
   * Get current master key (if unlocked)
   */
  getMasterKey(): MasterKey | null {
    if (this.masterKey && !this.isExpired(this.masterKey)) {
      return this.masterKey;
    }
    // Key is expired, clear it
    if (this.masterKey) {
      this.clearMasterKey();
    }
    return null;
  }

  /**
   * Set master key (on unlock)
   */
  setMasterKey(key: MasterKey): void {
    this.masterKey = key;
  }

  /**
   * Clear master key (on lock)
   */
  clearMasterKey(): void {
    // JavaScript can't wipe memory or a CryptoKey, and the garbage collector decides
    // when they go. The raw bytes are ours to overwrite, though, which is what lets
    // them outlive the session in a heap dump.
    if (this.masterKey) {
      this.masterKey.keyBytes.fill(0);
      this.masterKey.password = undefined;
      this.masterKey.derivedAt = 0;
    }
    this.masterKey = null;

    // Notify lock callbacks
    this.lockCallbacks.forEach(callback => callback());
  }

  /**
   * Check if application is locked
   */
  isLocked(): boolean {
    return this.masterKey === null;
  }

  /**
   * Check if key is expired (for future implementation)
   */
  private isExpired(_key: MasterKey): boolean {
    // For now, keys don't expire on their own
    // This could be extended to add a max session time
    return false;
  }

  /**
   * Subscribe to lock events
   */
  onLock(callback: () => void): () => void {
    this.lockCallbacks.add(callback);
    return () => {
      this.lockCallbacks.delete(callback);
    };
  }

  /**
   * Check if a key is valid (non-null and not expired)
   */
  isKeyValid(): boolean {
    return this.getMasterKey() !== null;
  }

  /**
   * Get time since key was derived (in milliseconds)
   */
  getKeyAge(): number | null {
    if (!this.masterKey) return null;
    return Date.now() - this.masterKey.derivedAt;
  }
}

/**
 * Singleton instance of key manager
 */
export const keyManager = new KeyManagerService();

// Re-export from centralized errors for backwards compatibility
export { ApplicationLockedError } from '../errors';
import { ApplicationLockedError } from '../errors';

/**
 * Get the master key or throw if application is locked
 * Use this helper to avoid repeating the null check pattern
 *
 * @throws {ApplicationLockedError} If application is locked
 * @returns The master key (guaranteed non-null)
 */
export function requireMasterKey(): CryptoKey {
  const masterKey = keyManager.getMasterKey();
  if (!masterKey) {
    throw new ApplicationLockedError();
  }
  return masterKey.key;
}
