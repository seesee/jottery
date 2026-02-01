/**
 * Tests for settingsRepository
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { settingsRepository } from './settingsRepository';
import { DEFAULT_SETTINGS, type KeyboardShortcuts, type ColorPalette } from '../types';
import { initTestDB, cleanupTestDB } from '../../test/db-utils';
import { keyManager } from './keyManager';
import { getDB, STORES } from './db';

describe('settingsRepository', () => {
  let masterKey: CryptoKey;

  beforeEach(async () => {
    const testDB = await initTestDB();
    masterKey = testDB.masterKey;

    // Set up authenticated key manager for encryption tests
    keyManager.setMasterKey({
      key: masterKey,
      keyBytes: new Uint8Array(32),
      derivedAt: Date.now(),
    });
  });

  afterEach(async () => {
    await cleanupTestDB();
  });

  describe('get', () => {
    it('should return default settings when none exist', async () => {
      const settings = await settingsRepository.get();
      expect(settings).toEqual(DEFAULT_SETTINGS);
    });

    it('should return stored settings when they exist', async () => {
      const customSettings = {
        ...DEFAULT_SETTINGS,
        theme: 'dark' as const,
        language: 'en-US',
      };
      await settingsRepository.update(customSettings);

      const retrieved = await settingsRepository.get();
      expect(retrieved).toEqual(customSettings);
    });

    it('should preserve all settings fields', async () => {
      const customSettings = {
        ...DEFAULT_SETTINGS,
        theme: 'light' as const,
        layoutMode: 'desktop' as const,
        sortOrder: 'alpha' as const,
        autoLockTimeout: 30,
        fontSize: 'large' as const,
        timezone: 'America/New_York',
        syncEnabled: true,
        syncEndpoint: 'https://sync.example.com',
        rememberPassword: true,
      };
      await settingsRepository.update(customSettings);

      const retrieved = await settingsRepository.get();
      expect(retrieved.theme).toBe('light');
      expect(retrieved.layoutMode).toBe('desktop');
      expect(retrieved.sortOrder).toBe('alpha');
      expect(retrieved.autoLockTimeout).toBe(30);
      expect(retrieved.fontSize).toBe('large');
      expect(retrieved.timezone).toBe('America/New_York');
      expect(retrieved.syncEnabled).toBe(true);
      expect(retrieved.syncEndpoint).toBe('https://sync.example.com');
      expect(retrieved.rememberPassword).toBe(true);
    });
  });

  describe('update', () => {
    it('should perform partial update', async () => {
      const updated = await settingsRepository.update({ theme: 'dark' });

      expect(updated.theme).toBe('dark');
      expect(updated.language).toBe(DEFAULT_SETTINGS.language);
      expect(updated.sortOrder).toBe(DEFAULT_SETTINGS.sortOrder);
    });

    it('should merge with existing settings', async () => {
      await settingsRepository.update({ theme: 'dark', language: 'en-US' });
      const updated = await settingsRepository.update({ sortOrder: 'alpha' });

      expect(updated.theme).toBe('dark');
      expect(updated.language).toBe('en-US');
      expect(updated.sortOrder).toBe('alpha');
    });

    it('should update multiple fields at once', async () => {
      const updated = await settingsRepository.update({
        theme: 'light',
        layoutMode: 'mobile',
        sortOrder: 'oldest',
        autoLockTimeout: 60,
      });

      expect(updated.theme).toBe('light');
      expect(updated.layoutMode).toBe('mobile');
      expect(updated.sortOrder).toBe('oldest');
      expect(updated.autoLockTimeout).toBe(60);
    });

    it('should handle empty update', async () => {
      const initial = await settingsRepository.get();
      const updated = await settingsRepository.update({});

      expect(updated).toEqual(initial);
    });

    it('should update sync settings', async () => {
      const updated = await settingsRepository.update({
        syncEnabled: true,
        syncEndpoint: 'https://sync.example.com',
      });

      expect(updated.syncEnabled).toBe(true);
      expect(updated.syncEndpoint).toBe('https://sync.example.com');
    });

    it('should update keyboard shortcuts', async () => {
      const customShortcuts: Partial<KeyboardShortcuts> = {
        newNote: { key: 'n', ctrl: true, alt: true },
        focusSearch: { key: 'f', ctrl: true },
      };
      const updated = await settingsRepository.update({
        keyboardShortcuts: customShortcuts as KeyboardShortcuts,
      });

      expect(updated.keyboardShortcuts).toBeDefined();
    });

    it('should update enabled syntax languages', async () => {
      const languages = ['javascript', 'typescript', 'rust'];
      const updated = await settingsRepository.update({
        enabledSyntaxLanguages: languages,
      });

      expect(updated.enabledSyntaxLanguages).toEqual(languages);
    });

    it('should persist updates across get calls', async () => {
      await settingsRepository.update({ theme: 'dark' });

      const retrieved1 = await settingsRepository.get();
      const retrieved2 = await settingsRepository.get();

      expect(retrieved1.theme).toBe('dark');
      expect(retrieved2.theme).toBe('dark');
    });

    it('should handle boolean toggles', async () => {
      await settingsRepository.update({ syncEnabled: false });
      let settings = await settingsRepository.get();
      expect(settings.syncEnabled).toBe(false);

      await settingsRepository.update({ syncEnabled: true });
      settings = await settingsRepository.get();
      expect(settings.syncEnabled).toBe(true);
    });

    it('should handle numeric values', async () => {
      const updated = await settingsRepository.update({ autoLockTimeout: 0 });
      expect(updated.autoLockTimeout).toBe(0);
    });

    it('should handle timezone updates', async () => {
      const timezones = ['UTC', 'America/New_York', 'Europe/London', 'Asia/Tokyo', 'local'];

      for (const timezone of timezones) {
        const updated = await settingsRepository.update({ timezone });
        expect(updated.timezone).toBe(timezone);
      }
    });

    it('should handle font size updates', async () => {
      const sizes = ['auto', 'small', 'medium', 'large'] as const;

      for (const fontSize of sizes) {
        const updated = await settingsRepository.update({ fontSize });
        expect(updated.fontSize).toBe(fontSize);
      }
    });

    it('should handle layout mode updates', async () => {
      const modes = ['auto', 'mobile', 'desktop'] as const;

      for (const layoutMode of modes) {
        const updated = await settingsRepository.update({ layoutMode });
        expect(updated.layoutMode).toBe(layoutMode);
      }
    });

    it('should handle sort order updates', async () => {
      const orders = ['recent', 'oldest', 'alpha', 'created'] as const;

      for (const sortOrder of orders) {
        const updated = await settingsRepository.update({ sortOrder });
        expect(updated.sortOrder).toBe(sortOrder);
      }
    });
  });

  describe('reset', () => {
    it('should restore default settings', async () => {
      await settingsRepository.update({
        theme: 'dark',
        language: 'en-US',
        sortOrder: 'alpha',
      });

      const reset = await settingsRepository.reset();
      expect(reset).toEqual(DEFAULT_SETTINGS);
    });

    it('should clear all custom settings', async () => {
      await settingsRepository.update({
        theme: 'light',
        layoutMode: 'desktop',
        sortOrder: 'oldest',
        autoLockTimeout: 60,
        fontSize: 'large',
        timezone: 'America/New_York',
        syncEnabled: true,
        syncEndpoint: 'https://sync.example.com',
        rememberPassword: true,
      });

      const reset = await settingsRepository.reset();

      expect(reset.theme).toBe(DEFAULT_SETTINGS.theme);
      expect(reset.layoutMode).toBe(DEFAULT_SETTINGS.layoutMode);
      expect(reset.sortOrder).toBe(DEFAULT_SETTINGS.sortOrder);
      expect(reset.autoLockTimeout).toBe(DEFAULT_SETTINGS.autoLockTimeout);
      expect(reset.fontSize).toBe(DEFAULT_SETTINGS.fontSize);
      expect(reset.timezone).toBe(DEFAULT_SETTINGS.timezone);
      expect(reset.syncEnabled).toBe(DEFAULT_SETTINGS.syncEnabled);
      expect(reset.syncEndpoint).toBe(DEFAULT_SETTINGS.syncEndpoint);
      expect(reset.rememberPassword).toBe(DEFAULT_SETTINGS.rememberPassword);
    });

    it('should persist reset across get calls', async () => {
      await settingsRepository.update({ theme: 'dark' });
      await settingsRepository.reset();

      const retrieved = await settingsRepository.get();
      expect(retrieved).toEqual(DEFAULT_SETTINGS);
    });

    it('should be idempotent', async () => {
      const reset1 = await settingsRepository.reset();
      const reset2 = await settingsRepository.reset();
      const reset3 = await settingsRepository.reset();

      expect(reset1).toEqual(DEFAULT_SETTINGS);
      expect(reset2).toEqual(DEFAULT_SETTINGS);
      expect(reset3).toEqual(DEFAULT_SETTINGS);
    });
  });

  describe('exists', () => {
    it('should return false when no settings exist', async () => {
      const exists = await settingsRepository.exists();
      expect(exists).toBe(false);
    });

    it('should return true after first update', async () => {
      await settingsRepository.update({ theme: 'dark' });

      const exists = await settingsRepository.exists();
      expect(exists).toBe(true);
    });

    it('should return true after reset', async () => {
      await settingsRepository.reset();

      const exists = await settingsRepository.exists();
      expect(exists).toBe(true);
    });

    it('should return true after multiple updates', async () => {
      await settingsRepository.update({ theme: 'dark' });
      await settingsRepository.update({ language: 'en-US' });
      await settingsRepository.update({ sortOrder: 'alpha' });

      const exists = await settingsRepository.exists();
      expect(exists).toBe(true);
    });
  });

  describe('edge cases', () => {
    it('should handle rapid successive updates', async () => {
      const updates = [
        settingsRepository.update({ theme: 'dark' }),
        settingsRepository.update({ language: 'en-US' }),
        settingsRepository.update({ sortOrder: 'alpha' }),
      ];

      await Promise.all(updates);

      const settings = await settingsRepository.get();
      // Last write wins for each field
      expect(settings.theme).toBeDefined();
      expect(settings.language).toBeDefined();
      expect(settings.sortOrder).toBeDefined();
    });

    it('should handle undefined optional fields', async () => {
      const updated = await settingsRepository.update({
        syncEndpoint: undefined,
        keyboardShortcuts: undefined,
        enabledSyntaxLanguages: undefined,
      });

      expect(updated.syncEndpoint).toBeUndefined();
      expect(updated.keyboardShortcuts).toBeUndefined();
      expect(updated.enabledSyntaxLanguages).toBeUndefined();
    });

    it('should handle very long autoLockTimeout', async () => {
      const updated = await settingsRepository.update({ autoLockTimeout: 999999 });
      expect(updated.autoLockTimeout).toBe(999999);
    });

    it('should handle zero autoLockTimeout', async () => {
      const updated = await settingsRepository.update({ autoLockTimeout: 0 });
      expect(updated.autoLockTimeout).toBe(0);
    });

    it('should preserve default keyboard shortcuts', async () => {
      const settings = await settingsRepository.get();
      expect(settings.keyboardShortcuts).toEqual(DEFAULT_SETTINGS.keyboardShortcuts);
    });

    it('should preserve default syntax languages', async () => {
      const settings = await settingsRepository.get();
      expect(settings.enabledSyntaxLanguages).toEqual(DEFAULT_SETTINGS.enabledSyntaxLanguages);
    });
  });

  describe('integration scenarios', () => {
    it('should support full settings workflow', async () => {
      // Check initial state
      const exists1 = await settingsRepository.exists();
      expect(exists1).toBe(false);

      // Get defaults
      const defaults = await settingsRepository.get();
      expect(defaults).toEqual(DEFAULT_SETTINGS);

      // Update some settings
      await settingsRepository.update({
        theme: 'dark',
        language: 'en-US',
        syncEnabled: true,
      });

      // Verify exists
      const exists2 = await settingsRepository.exists();
      expect(exists2).toBe(true);

      // Get updated settings
      const updated = await settingsRepository.get();
      expect(updated.theme).toBe('dark');
      expect(updated.language).toBe('en-US');
      expect(updated.syncEnabled).toBe(true);

      // Reset
      await settingsRepository.reset();

      // Verify reset
      const reset = await settingsRepository.get();
      expect(reset).toEqual(DEFAULT_SETTINGS);
    });

    it('should handle theme switching', async () => {
      await settingsRepository.update({ theme: 'light' });
      let settings = await settingsRepository.get();
      expect(settings.theme).toBe('light');

      await settingsRepository.update({ theme: 'dark' });
      settings = await settingsRepository.get();
      expect(settings.theme).toBe('dark');

      await settingsRepository.update({ theme: 'auto' });
      settings = await settingsRepository.get();
      expect(settings.theme).toBe('auto');
    });

    it('should handle sync configuration', async () => {
      // Disable sync
      await settingsRepository.update({ syncEnabled: false });
      let settings = await settingsRepository.get();
      expect(settings.syncEnabled).toBe(false);

      // Enable sync with endpoint
      await settingsRepository.update({
        syncEnabled: true,
        syncEndpoint: 'https://sync.example.com',
      });
      settings = await settingsRepository.get();
      expect(settings.syncEnabled).toBe(true);
      expect(settings.syncEndpoint).toBe('https://sync.example.com');

      // Disable sync but keep endpoint
      await settingsRepository.update({ syncEnabled: false });
      settings = await settingsRepository.get();
      expect(settings.syncEnabled).toBe(false);
      expect(settings.syncEndpoint).toBe('https://sync.example.com');
    });

    it('should handle language changes', async () => {
      const languages = ['en-GB', 'en-US', 'es', 'fr', 'de'];

      for (const language of languages) {
        await settingsRepository.update({ language });
        const settings = await settingsRepository.get();
        expect(settings.language).toBe(language);
      }
    });
  });

  describe('colorPalette encryption', () => {
    it('should encrypt displayNames in colorPalette when stored', async () => {
      const customPalette: ColorPalette = {
        red: {
          light: '#FFE5E5',
          dark: '#5C1A1A',
          displayName: 'Project Alpha', // Should be encrypted
        },
        blue: {
          light: '#E5F0FF',
          dark: '#1A3A5C',
          displayName: 'Client Beta', // Should be encrypted
        },
        green: {
          light: '#E5F5E5',
          dark: '#1A4D1A',
          // No displayName - should work fine
        },
      };

      await settingsRepository.update({ colorPalette: customPalette });

      // Check raw storage
      const db = await getDB();
      const stored = await db.get(STORES.SETTINGS, 'user-settings');

      // DisplayNames should be encrypted JSON strings
      expect(stored?.colorPalette?.red?.displayName).not.toBe('Project Alpha');
      expect(stored?.colorPalette?.red?.displayName?.startsWith('{')).toBe(true);
      expect(stored?.colorPalette?.red?.displayName).toContain('"ciphertext"');

      expect(stored?.colorPalette?.blue?.displayName).not.toBe('Client Beta');
      expect(stored?.colorPalette?.blue?.displayName?.startsWith('{')).toBe(true);

      // Green has no displayName, should be undefined
      expect(stored?.colorPalette?.green?.displayName).toBeUndefined();

      // Light/dark colors should NOT be encrypted
      expect(stored?.colorPalette?.red?.light).toBe('#FFE5E5');
      expect(stored?.colorPalette?.red?.dark).toBe('#5C1A1A');
    });

    it('should decrypt displayNames when retrieved', async () => {
      const customPalette: ColorPalette = {
        red: {
          light: '#FFE5E5',
          dark: '#5C1A1A',
          displayName: 'Important Project',
        },
        yellow: {
          light: '#FFFACD',
          dark: '#5C5520',
          displayName: 'Follow-up Tasks',
        },
      };

      await settingsRepository.update({ colorPalette: customPalette });
      const retrieved = await settingsRepository.get();

      expect(retrieved.colorPalette?.red?.displayName).toBe('Important Project');
      expect(retrieved.colorPalette?.yellow?.displayName).toBe('Follow-up Tasks');
    });

    it('should handle colorPalette without displayNames', async () => {
      const simplePalette: ColorPalette = {
        red: { light: '#FFE5E5', dark: '#5C1A1A' },
        blue: { light: '#E5F0FF', dark: '#1A3A5C' },
      };

      const updated = await settingsRepository.update({ colorPalette: simplePalette });

      expect(updated.colorPalette?.red?.light).toBe('#FFE5E5');
      expect(updated.colorPalette?.red?.displayName).toBeUndefined();
      expect(updated.colorPalette?.blue?.light).toBe('#E5F0FF');
      expect(updated.colorPalette?.blue?.displayName).toBeUndefined();
    });

    it('should handle legacy unencrypted displayNames', async () => {
      // Manually insert unencrypted data (simulating old data)
      const db = await getDB();
      const legacySettings = {
        ...DEFAULT_SETTINGS,
        colorPalette: {
          red: {
            light: '#FFE5E5',
            dark: '#5C1A1A',
            displayName: 'Legacy Name', // Not encrypted
          },
        },
      };
      await db.put(STORES.SETTINGS, legacySettings, 'user-settings');

      // Should read legacy data without errors
      const retrieved = await settingsRepository.get();
      expect(retrieved.colorPalette?.red?.displayName).toBe('Legacy Name');
    });

    it('should encrypt displayNames with special characters', async () => {
      const specialPalette: ColorPalette = {
        purple: {
          light: '#F0E5FF',
          dark: '#3A1A5C',
          displayName: 'Clîënt Prøjéct 🎨 with émojis & spëcial çhars',
        },
      };

      await settingsRepository.update({ colorPalette: specialPalette });
      const retrieved = await settingsRepository.get();

      expect(retrieved.colorPalette?.purple?.displayName).toBe(
        'Clîënt Prøjéct 🎨 with émojis & spëcial çhars'
      );
    });

    it('should update displayNames correctly', async () => {
      // Create palette with displayName
      const initialPalette: ColorPalette = {
        red: {
          light: '#FFE5E5',
          dark: '#5C1A1A',
          displayName: 'Original Name',
        },
      };
      await settingsRepository.update({ colorPalette: initialPalette });

      // Update displayName
      const updatedPalette: ColorPalette = {
        red: {
          light: '#FFE5E5',
          dark: '#5C1A1A',
          displayName: 'Updated Name',
        },
      };
      await settingsRepository.update({ colorPalette: updatedPalette });

      // Verify encryption
      const db = await getDB();
      const stored = await db.get(STORES.SETTINGS, 'user-settings');
      expect(stored?.colorPalette?.red?.displayName).not.toBe('Updated Name');
      expect(stored?.colorPalette?.red?.displayName?.startsWith('{')).toBe(true);

      // Verify decryption
      const retrieved = await settingsRepository.get();
      expect(retrieved.colorPalette?.red?.displayName).toBe('Updated Name');
    });

    it('should handle mixed encrypted and unencrypted colors in palette', async () => {
      const mixedPalette: ColorPalette = {
        red: {
          light: '#FFE5E5',
          dark: '#5C1A1A',
          displayName: 'Encrypted Name',
        },
        blue: {
          light: '#E5F0FF',
          dark: '#1A3A5C',
          // No displayName
        },
        green: {
          light: '#E5F5E5',
          dark: '#1A4D1A',
          displayName: 'Another Encrypted',
        },
      };

      await settingsRepository.update({ colorPalette: mixedPalette });
      const retrieved = await settingsRepository.get();

      expect(retrieved.colorPalette?.red?.displayName).toBe('Encrypted Name');
      expect(retrieved.colorPalette?.blue?.displayName).toBeUndefined();
      expect(retrieved.colorPalette?.green?.displayName).toBe('Another Encrypted');
    });

    it('should fail gracefully when app is locked during update', async () => {
      // Clear master key to simulate locked state
      keyManager.clearMasterKey();

      const palette: ColorPalette = {
        red: {
          light: '#FFE5E5',
          dark: '#5C1A1A',
          displayName: 'Test',
        },
      };

      await expect(settingsRepository.update({ colorPalette: palette })).rejects.toThrow(
        'Application is locked'
      );

      // Restore key for cleanup
      keyManager.setMasterKey({
        key: masterKey,
        keyBytes: new Uint8Array(32),
        derivedAt: Date.now(),
      });
    });

    it('should return as-is when app is locked during get', async () => {
      // Store encrypted palette while unlocked
      const palette: ColorPalette = {
        red: {
          light: '#FFE5E5',
          dark: '#5C1A1A',
          displayName: 'Test Name',
        },
      };
      await settingsRepository.update({ colorPalette: palette });

      // Clear master key to simulate locked state
      keyManager.clearMasterKey();

      // Should return encrypted displayName (can't decrypt without key)
      const retrieved = await settingsRepository.get();
      expect(retrieved.colorPalette?.red?.displayName).not.toBe('Test Name');
      expect(retrieved.colorPalette?.red?.displayName?.startsWith('{')).toBe(true);

      // Restore key for cleanup
      keyManager.setMasterKey({
        key: masterKey,
        keyBytes: new Uint8Array(32),
        derivedAt: Date.now(),
      });
    });

    it('should preserve tagColors alongside colorPalette', async () => {
      const palette: ColorPalette = {
        red: {
          light: '#FFE5E5',
          dark: '#5C1A1A',
          displayName: 'Important',
        },
      };

      const tagColors = {
        work: 'blue',
        personal: 'green',
        urgent: 'red',
      };

      await settingsRepository.update({
        colorPalette: palette,
        tagColors: tagColors,
      });

      const retrieved = await settingsRepository.get();

      expect(retrieved.colorPalette?.red?.displayName).toBe('Important');
      expect(retrieved.tagColors).toEqual(tagColors);
    });
  });
});
