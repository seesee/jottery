/**
 * Password storage service for "Remember Password" feature
 * WARNING: This stores the password in localStorage which is INSECURE
 * Password is base64 encoded for minimal obfuscation (NOT encryption)
 * Only use when user explicitly opts in with full understanding of risks
 */

const STORAGE_KEY = 'jottery_stored_password';

/**
 * Store password in localStorage with base64 encoding
 * NOTE: Base64 is NOT encryption, only basic obfuscation
 * @security This is an intentional design choice for user convenience.
 * Users are explicitly warned about the security implications.
 */
export function storePassword(password: string): void {
  try {
    // Base64 encode for minimal obfuscation (not security)
    const encoded = btoa(password);
    localStorage.setItem(STORAGE_KEY, encoded);
  } catch (error) {
    console.error('Failed to store password:', error);
    throw new Error('Failed to store password in localStorage');
  }
}

/**
 * Retrieve and decode stored password from localStorage
 */
export function getStoredPassword(): string | null {
  try {
    const encoded = localStorage.getItem(STORAGE_KEY);
    if (!encoded) return null;

    // Base64 decode
    try {
      return atob(encoded);
    } catch (decodeError) {
      console.error('Failed to decode stored password:', decodeError);
      // Clear corrupted data
      clearStoredPassword();
      return null;
    }
  } catch (error) {
    console.error('Failed to retrieve stored password:', error);
    return null;
  }
}

/**
 * Remove stored password from localStorage
 */
export function clearStoredPassword(): void {
  try {
    localStorage.removeItem(STORAGE_KEY);
  } catch (error) {
    console.error('Failed to clear stored password:', error);
  }
}

/**
 * Check if password is currently stored
 */
export function hasStoredPassword(): boolean {
  return getStoredPassword() !== null;
}

/**
 * Password storage service interface
 */
export const passwordStorageService = {
  store: storePassword,
  get: getStoredPassword,
  clear: clearStoredPassword,
  hasStored: hasStoredPassword,
};
