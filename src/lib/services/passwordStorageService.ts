/**
 * Password storage service for "Remember Password" feature
 * WARNING: This stores the password in localStorage which is INSECURE
 * Only use when user explicitly opts in with full understanding of risks
 */

const STORAGE_KEY = 'jottery_stored_password';

/**
 * Store password in localStorage (INSECURE)
 */
export function storePassword(password: string): void {
  try {
    localStorage.setItem(STORAGE_KEY, password);
  } catch (error) {
    console.error('Failed to store password:', error);
    throw new Error('Failed to store password in localStorage');
  }
}

/**
 * Retrieve stored password from localStorage
 */
export function getStoredPassword(): string | null {
  try {
    return localStorage.getItem(STORAGE_KEY);
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
