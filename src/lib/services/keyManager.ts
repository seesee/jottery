/**
 * Key manager service for handling master key lifecycle
 * Manages locking, unlocking, and auto-lock timeout
 */

import type { KeyManager, MasterKey } from '../types';

class KeyManagerService implements KeyManager {
  private masterKey: MasterKey | null = null;
  private autoLockTimer: number | null = null;
  private activityCallbacks: Set<() => void> = new Set();
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
    this.notifyActivity();
  }

  /**
   * Clear master key (on lock)
   */
  clearMasterKey(): void {
    // Securely clear the key from memory
    this.masterKey = null;

    // Stop auto-lock timer
    if (this.autoLockTimer !== null) {
      clearTimeout(this.autoLockTimer);
      this.autoLockTimer = null;
    }

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
   * Register activity (for auto-lock timeout)
   */
  registerActivity(): void {
    if (this.isLocked()) return;

    this.notifyActivity();
    this.resetAutoLockTimer();
  }

  /**
   * Start auto-lock timer
   */
  startAutoLock(timeoutMinutes: number): void {
    this.stopAutoLock();

    if (timeoutMinutes <= 0) return;

    const timeoutMs = timeoutMinutes * 60 * 1000;
    this.autoLockTimer = window.setTimeout(() => {
      this.clearMasterKey();
    }, timeoutMs);
  }

  /**
   * Stop auto-lock timer
   */
  stopAutoLock(): void {
    if (this.autoLockTimer !== null) {
      clearTimeout(this.autoLockTimer);
      this.autoLockTimer = null;
    }
  }

  /**
   * Reset auto-lock timer on activity
   */
  private resetAutoLockTimer(): void {
    // The timer will be restarted by the component that manages settings
    // This just ensures we clear the old one
    if (this.autoLockTimer !== null) {
      clearTimeout(this.autoLockTimer);
      this.autoLockTimer = null;
    }
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
   * Notify activity callbacks
   */
  private notifyActivity(): void {
    this.activityCallbacks.forEach(callback => callback());
  }

  /**
   * Subscribe to activity events
   */
  onActivity(callback: () => void): () => void {
    this.activityCallbacks.add(callback);
    return () => {
      this.activityCallbacks.delete(callback);
    };
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

/**
 * Setup global activity listeners for auto-lock
 */
export function setupActivityListeners(timeoutMinutes: number): void {
  const events = ['mousedown', 'keydown', 'scroll', 'touchstart'];

  const handler = () => {
    keyManager.registerActivity();
  };

  events.forEach(event => {
    window.addEventListener(event, handler, { passive: true });
  });

  // Start initial auto-lock timer
  keyManager.startAutoLock(timeoutMinutes);
}

/**
 * Remove activity listeners
 */
export function removeActivityListeners(): void {
  const events = ['mousedown', 'keydown', 'scroll', 'touchstart'];

  const handler = () => {
    keyManager.registerActivity();
  };

  events.forEach(event => {
    window.removeEventListener(event, handler);
  });

  keyManager.stopAutoLock();
}
