/**
 * Settings repository implementation for IndexedDB
 */

import type { SettingsRepository, UserSettings } from '../types';
import { DEFAULT_SETTINGS } from '../types';
import { getDB, STORES } from './db';

const SETTINGS_KEY = 'user-settings';

class IndexedDBSettingsRepository implements SettingsRepository {
  /**
   * Get current user settings
   */
  async get(): Promise<UserSettings> {
    const db = getDB();
    const settings = await db.get(STORES.SETTINGS, SETTINGS_KEY);
    return settings || DEFAULT_SETTINGS;
  }

  /**
   * Update user settings (partial update supported)
   */
  async update(settings: Partial<UserSettings>): Promise<UserSettings> {
    const db = getDB();
    const current = await this.get();
    const updated = { ...current, ...settings };
    await db.put(STORES.SETTINGS, updated, SETTINGS_KEY);
    return updated;
  }

  /**
   * Reset to default settings
   */
  async reset(): Promise<UserSettings> {
    const db = getDB();
    await db.put(STORES.SETTINGS, DEFAULT_SETTINGS, SETTINGS_KEY);
    return DEFAULT_SETTINGS;
  }

  /**
   * Check if settings exist
   */
  async exists(): Promise<boolean> {
    const db = getDB();
    const settings = await db.get(STORES.SETTINGS, SETTINGS_KEY);
    return settings !== undefined;
  }
}

/**
 * Singleton instance
 */
export const settingsRepository = new IndexedDBSettingsRepository();
