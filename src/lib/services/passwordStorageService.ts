/**
 * Password storage service for the "Remember Password" feature.
 *
 * Storage backend: localStorage with base64 encoding. This is obfuscation, not
 * encryption — anything with access to the origin's storage can recover the
 * password, and the UI warns about that where the feature is offered.
 *
 * There was previously an OS-keychain path used when running inside the Electron
 * desktop build. That build has been retired, so the keychain branch was
 * unreachable and has been removed rather than left as dead code.
 */

const STORAGE_KEY = 'jottery_stored_password';

/**
 * Store the password.
 *
 * @throws if localStorage is unavailable or full.
 */
export async function storePassword(password: string): Promise<void> {
  try {
    localStorage.setItem(STORAGE_KEY, btoa(password));
  } catch (error) {
    console.error('Failed to store password:', error);
    throw new Error('Failed to store password in localStorage');
  }
}

/**
 * Retrieve the stored password, or null if none is stored.
 *
 * A value that fails to decode is treated as corrupt and cleared, so a bad entry
 * cannot wedge the unlock screen on every load.
 */
export async function getStoredPassword(): Promise<string | null> {
  try {
    const encoded = localStorage.getItem(STORAGE_KEY);
    if (!encoded) return null;

    try {
      return atob(encoded);
    } catch (decodeError) {
      console.error('Failed to decode stored password:', decodeError);
      clearStoredPasswordSync();
      return null;
    }
  } catch (error) {
    console.error('Failed to retrieve stored password:', error);
    return null;
  }
}

/**
 * Remove the stored password.
 */
export async function clearStoredPassword(): Promise<void> {
  clearStoredPasswordSync();
}

/**
 * Synchronous localStorage clear (for internal use)
 */
function clearStoredPasswordSync(): void {
  try {
    localStorage.removeItem(STORAGE_KEY);
  } catch (error) {
    console.error('Failed to clear stored password:', error);
  }
}

/**
 * Check if a password is currently stored
 */
export async function hasStoredPassword(): Promise<boolean> {
  const password = await getStoredPassword();
  return password !== null;
}

/**
 * Password storage service interface (async version)
 */
export const passwordStorageService = {
  store: storePassword,
  get: getStoredPassword,
  clear: clearStoredPassword,
  hasStored: hasStoredPassword,
};
