import { ipcMain } from 'electron';

// Keytar is optional - only available when native modules are built
let keytar: typeof import('keytar') | null = null;

/**
 * Attempt to load keytar for secure OS keychain access.
 * Falls back gracefully if not available.
 */
async function loadKeytar(): Promise<boolean> {
  if (keytar !== null) {
    return true;
  }

  try {
    // Dynamic import to handle missing native module
    keytar = await import('keytar');
    return true;
  } catch {
    console.warn('keytar not available - password storage will use fallback');
    return false;
  }
}

/**
 * Setup IPC handlers for secure password storage via OS keychain.
 *
 * Uses keytar to store passwords securely:
 * - macOS: Keychain
 * - Windows: Credential Vault
 * - Linux: libsecret (GNOME Keyring, KWallet, etc.)
 */
export function setupPasswordStorage(): void {
  // Check if keytar is available
  ipcMain.handle('password:isAvailable', async () => {
    return await loadKeytar();
  });

  // Get a password from the keychain
  ipcMain.handle(
    'password:get',
    async (_event, service: string, account: string): Promise<string | null> => {
      const available = await loadKeytar();
      if (!available || !keytar) {
        return null;
      }

      try {
        return await keytar.getPassword(service, account);
      } catch (error) {
        console.error('Failed to get password:', error);
        return null;
      }
    }
  );

  // Set a password in the keychain
  ipcMain.handle(
    'password:set',
    async (_event, service: string, account: string, password: string): Promise<void> => {
      const available = await loadKeytar();
      if (!available || !keytar) {
        throw new Error('Keychain not available');
      }

      try {
        await keytar.setPassword(service, account, password);
      } catch (error) {
        console.error('Failed to set password:', error);
        throw error;
      }
    }
  );

  // Delete a password from the keychain
  ipcMain.handle(
    'password:delete',
    async (_event, service: string, account: string): Promise<boolean> => {
      const available = await loadKeytar();
      if (!available || !keytar) {
        return false;
      }

      try {
        return await keytar.deletePassword(service, account);
      } catch (error) {
        console.error('Failed to delete password:', error);
        return false;
      }
    }
  );
}
