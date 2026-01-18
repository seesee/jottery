/**
 * Session storage service for "Persist Session" feature
 * Stores password temporarily in sessionStorage (tab-scoped, clears on browser close)
 * More secure than "Remember Password" - expires automatically and clears when tab closes
 * Password is base64 encoded for minimal obfuscation (NOT encryption)
 */

const PASSWORD_KEY = 'jottery_session_password';
const TIMESTAMP_KEY = 'jottery_session_timestamp';

/**
 * Check if sessionStorage is available
 * May not be available for file:// URLs or in private browsing mode
 */
export function isSessionStorageAvailable(): boolean {
  try {
    const testKey = '__jottery_session_test__';
    sessionStorage.setItem(testKey, 'test');
    sessionStorage.removeItem(testKey);
    return true;
  } catch {
    return false;
  }
}

/**
 * Store password in sessionStorage with base64 encoding and timestamp
 * NOTE: Base64 is NOT encryption, only basic obfuscation
 */
export function storeSessionPassword(password: string): void {
  if (!isSessionStorageAvailable()) {
    console.warn('[SessionStorage] sessionStorage not available');
    return;
  }

  try {
    // Base64 encode for minimal obfuscation (not security)
    const encoded = btoa(password);
    sessionStorage.setItem(PASSWORD_KEY, encoded);
    sessionStorage.setItem(TIMESTAMP_KEY, Date.now().toString());
  } catch (error) {
    console.error('[SessionStorage] Failed to store session password:', error);
  }
}

/**
 * Retrieve stored password from sessionStorage if not expired
 * @param timeoutMinutes - Session expiry timeout in minutes
 * @returns The stored password or null if expired/invalid
 */
export function getSessionPassword(timeoutMinutes: number): string | null {
  if (!isSessionStorageAvailable()) {
    return null;
  }

  try {
    const encoded = sessionStorage.getItem(PASSWORD_KEY);
    const timestampStr = sessionStorage.getItem(TIMESTAMP_KEY);

    if (!encoded || !timestampStr) {
      return null;
    }

    // Check expiry
    const timestamp = parseInt(timestampStr, 10);
    if (isNaN(timestamp)) {
      clearSessionPassword();
      return null;
    }

    const expiryMs = timeoutMinutes * 60 * 1000;
    const elapsed = Date.now() - timestamp;

    if (elapsed > expiryMs) {
      console.log('[SessionStorage] Session expired');
      clearSessionPassword();
      return null;
    }

    // Decode and return
    try {
      return atob(encoded);
    } catch (decodeError) {
      console.error('[SessionStorage] Failed to decode session password:', decodeError);
      clearSessionPassword();
      return null;
    }
  } catch (error) {
    console.error('[SessionStorage] Failed to retrieve session password:', error);
    return null;
  }
}

/**
 * Refresh the session timestamp (call on successful unlock to extend session)
 */
export function refreshSessionTimestamp(): void {
  if (!isSessionStorageAvailable()) {
    return;
  }

  try {
    const encoded = sessionStorage.getItem(PASSWORD_KEY);
    if (encoded) {
      sessionStorage.setItem(TIMESTAMP_KEY, Date.now().toString());
    }
  } catch (error) {
    console.error('[SessionStorage] Failed to refresh session timestamp:', error);
  }
}

/**
 * Remove stored session from sessionStorage
 */
export function clearSessionPassword(): void {
  if (!isSessionStorageAvailable()) {
    return;
  }

  try {
    sessionStorage.removeItem(PASSWORD_KEY);
    sessionStorage.removeItem(TIMESTAMP_KEY);
  } catch (error) {
    console.error('[SessionStorage] Failed to clear session password:', error);
  }
}

/**
 * Check if a valid (non-expired) session exists
 * @param timeoutMinutes - Session expiry timeout in minutes
 */
export function hasValidSession(timeoutMinutes: number): boolean {
  return getSessionPassword(timeoutMinutes) !== null;
}

/**
 * Session storage service interface
 */
export const sessionStorageService = {
  isAvailable: isSessionStorageAvailable,
  store: storeSessionPassword,
  get: getSessionPassword,
  refresh: refreshSessionTimestamp,
  clear: clearSessionPassword,
  hasValid: hasValidSession,
};
