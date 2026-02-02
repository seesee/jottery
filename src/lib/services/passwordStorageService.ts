import { isElectron } from '../utils/device';

/**
 * Password storage service for "Remember Password" feature
 *
 * Storage backends:
 * - Electron: OS keychain (secure) via keytar
 * - Web: localStorage with base64 encoding (INSECURE, user warned)
 */

const STORAGE_KEY = 'jottery_stored_password';
const KEYCHAIN_SERVICE = 'io.jottery.app';
const KEYCHAIN_ACCOUNT = 'master-password';

// Track if keychain is available (cached after first check)
let keychainAvailable: boolean | null = null;

/**
 * Check if secure keychain storage is available (Electron with keytar)
 */
async function isKeychainAvailable(): Promise<boolean> {
  if (keychainAvailable !== null) {
    return keychainAvailable;
  }

  if (!isElectron() || !window.electronAPI?.password) {
    keychainAvailable = false;
    return false;
  }

  try {
    keychainAvailable = await window.electronAPI.password.isAvailable();
    return keychainAvailable;
  } catch {
    keychainAvailable = false;
    return false;
  }
}

/**
 * Store password securely
 * - Electron: Uses OS keychain (macOS Keychain, Windows Credential Vault, Linux libsecret)
 * - Web: Uses localStorage with base64 encoding (user is warned about insecurity)
 */
export async function storePassword(password: string): Promise<void> {
  const useKeychain = await isKeychainAvailable();

  if (useKeychain && window.electronAPI?.password) {
    try {
      await window.electronAPI.password.set(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT, password);
      // Clear any legacy localStorage entry
      localStorage.removeItem(STORAGE_KEY);
      return;
    } catch (error) {
      console.error('Failed to store password in keychain:', error);
      throw new Error('Failed to store password securely');
    }
  }

  // Fallback to localStorage (web)
  try {
    const encoded = btoa(password);
    localStorage.setItem(STORAGE_KEY, encoded);
  } catch (error) {
    console.error('Failed to store password:', error);
    throw new Error('Failed to store password in localStorage');
  }
}

/**
 * Retrieve stored password
 */
export async function getStoredPassword(): Promise<string | null> {
  const useKeychain = await isKeychainAvailable();

  if (useKeychain && window.electronAPI?.password) {
    try {
      return await window.electronAPI.password.get(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT);
    } catch (error) {
      console.error('Failed to retrieve password from keychain:', error);
      return null;
    }
  }

  // Fallback to localStorage (web)
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
 * Remove stored password
 */
export async function clearStoredPassword(): Promise<void> {
  const useKeychain = await isKeychainAvailable();

  if (useKeychain && window.electronAPI?.password) {
    try {
      await window.electronAPI.password.delete(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT);
    } catch (error) {
      console.error('Failed to clear password from keychain:', error);
    }
  }

  // Always clear localStorage as well (migration cleanup)
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
 * Check if password is currently stored
 */
export async function hasStoredPassword(): Promise<boolean> {
  const password = await getStoredPassword();
  return password !== null;
}

/**
 * Check if storage is using secure keychain
 */
export async function isUsingSecureStorage(): Promise<boolean> {
  return await isKeychainAvailable();
}

/**
 * Password storage service interface (async version)
 */
export const passwordStorageService = {
  store: storePassword,
  get: getStoredPassword,
  clear: clearStoredPassword,
  hasStored: hasStoredPassword,
  isSecure: isUsingSecureStorage,
};
