/**
 * Settings repository implementation for IndexedDB
 */

import type { SettingsRepository, UserSettings, ColorPalette } from '../types';
import { DEFAULT_SETTINGS } from '../types';
import { getDB, STORES } from './db';
import { keyManager } from './keyManager';
import { cryptoService, decryptJSON, encryptJSON } from './crypto';
import { ApplicationLockedError } from '../errors';
import type { EncryptionResult } from '../types/crypto';

const SETTINGS_KEY = 'user-settings';

/**
 * Encrypt displayNames in color palette
 */
async function encryptColorPalette(palette?: ColorPalette): Promise<ColorPalette | undefined> {
  if (!palette) return palette;

  const masterKey = keyManager.getMasterKey();
  if (!masterKey) {
    throw new ApplicationLockedError();
  }

  const encrypted: ColorPalette = {};

  for (const [colorKey, colorDef] of Object.entries(palette)) {
    const typedColorDef = colorDef as { light: string; dark: string; displayName?: string };
    encrypted[colorKey] = { ...typedColorDef };

    // Encrypt displayName if present
    if (typedColorDef.displayName) {
      const encryptedDisplayName = await cryptoService.encryptText(
        typedColorDef.displayName,
        masterKey.key
      );
      encrypted[colorKey].displayName = JSON.stringify(encryptedDisplayName);
    }
  }

  return encrypted;
}

/**
 * Decrypt a colour's display name. Older vaults stored these as plaintext, and
 * since the palette never leaves this device they can still be read. A value that
 * is ciphertext but fails to decrypt is an error, so it is dropped, not shown.
 */
async function decryptDisplayName(value: string, key: CryptoKey): Promise<string | undefined> {
  let parsed: { ciphertext?: unknown; iv?: unknown } | null;
  try {
    parsed = JSON.parse(value);
  } catch {
    return value;
  }

  if (typeof parsed?.ciphertext !== 'string' || typeof parsed?.iv !== 'string') {
    return value;
  }

  try {
    return await cryptoService.decryptText(parsed as EncryptionResult, key);
  } catch (error) {
    console.warn('[SettingsRepository] Colour name failed to decrypt:', error);
    return undefined;
  }
}

/**
 * Decrypt displayNames in color palette
 */
async function decryptColorPalette(palette?: ColorPalette): Promise<ColorPalette | undefined> {
  if (!palette) return palette;

  const masterKey = keyManager.getMasterKey();
  if (!masterKey) {
    // If locked, return palette with encrypted displayNames
    return palette;
  }

  const decrypted: ColorPalette = {};

  for (const [colorKey, colorDef] of Object.entries(palette)) {
    const typedColorDef = colorDef as { light: string; dark: string; displayName?: string };
    decrypted[colorKey] = { ...typedColorDef };

    // Decrypt displayName if present
    if (typedColorDef.displayName) {
      decrypted[colorKey].displayName = await decryptDisplayName(typedColorDef.displayName, masterKey.key);
    }
  }

  return decrypted;
}

/**
 * Settings that say what the notes are about: tag names, past searches, snippets of
 * text. They are held in one encrypted value at rest, because most other settings
 * are needed before unlock and this record is otherwise readable from the browser
 * profile. Until unlock they read as their defaults.
 */
const PRIVATE_FIELDS = ['recentSearches', 'tagColors', 'quickCommandsList'] as const;

type StoredSettings = Partial<UserSettings> & { privateSettings?: string };
type PrivateSettings = Partial<Pick<UserSettings, typeof PRIVATE_FIELDS[number]>>;

function privatePartOf(settings: Partial<UserSettings>): PrivateSettings {
  const part: Record<string, unknown> = {};
  for (const field of PRIVATE_FIELDS) {
    if (field in settings) part[field] = settings[field];
  }
  return part as PrivateSettings;
}

async function readPrivateSettings(stored: StoredSettings): Promise<PrivateSettings> {
  const masterKey = keyManager.getMasterKey();
  if (!stored.privateSettings || !masterKey) {
    return {};
  }

  try {
    return await decryptJSON<PrivateSettings>(JSON.parse(stored.privateSettings), masterKey.key);
  } catch (error) {
    console.warn('[SettingsRepository] Private settings failed to decrypt:', error);
    return {};
  }
}

/**
 * Move private fields into the encrypted value, along with any an older version left
 * in plaintext. Without the key they can't be stored, and stay as they are.
 */
async function withPrivateFieldsEncrypted(stored: StoredSettings, changes: PrivateSettings): Promise<StoredSettings> {
  const masterKey = keyManager.getMasterKey();
  const plaintext = privatePartOf(stored);
  if (!masterKey || Object.keys(plaintext).length + Object.keys(changes).length === 0) {
    return stored;
  }

  const result = { ...stored };
  for (const field of PRIVATE_FIELDS) {
    delete result[field];
  }

  const merged = { ...plaintext, ...(await readPrivateSettings(stored)), ...changes };
  result.privateSettings = JSON.stringify(await encryptJSON(merged, masterKey.key));
  return result;
}

class IndexedDBSettingsRepository implements SettingsRepository {
  /**
   * Get current user settings
   * Always merges with DEFAULT_SETTINGS to ensure new fields exist
   */
  async get(): Promise<UserSettings> {
    const db = getDB();
    const stored: StoredSettings | undefined = await db.get(STORES.SETTINGS, SETTINGS_KEY);
    const { privateSettings: _encrypted, ...settings } = stored ?? {};
    // Merge with defaults to ensure new fields (like colorPalette) exist even in old databases
    const merged: UserSettings = stored
      ? { ...DEFAULT_SETTINGS, ...settings, ...(await readPrivateSettings(stored)) }
      : DEFAULT_SETTINGS;

    // Decrypt colorPalette displayNames
    if (merged.colorPalette) {
      merged.colorPalette = await decryptColorPalette(merged.colorPalette);
    }

    return merged;
  }

  /**
   * Update user settings (partial update supported)
   */
  async update(settings: Partial<UserSettings>): Promise<UserSettings> {
    const db = getDB();
    const current: StoredSettings = await db.get(STORES.SETTINGS, SETTINGS_KEY) || DEFAULT_SETTINGS;

    // Encrypt colorPalette displayNames before saving
    const settingsToSave = { ...settings };
    if (settingsToSave.colorPalette) {
      settingsToSave.colorPalette = await encryptColorPalette(settingsToSave.colorPalette);
    }

    const changes = privatePartOf(settingsToSave);
    const publicChanges: Partial<UserSettings> = { ...settingsToSave };
    for (const field of PRIVATE_FIELDS) {
      delete publicChanges[field];
    }

    const toStore = await withPrivateFieldsEncrypted({ ...current, ...publicChanges }, changes);
    await db.put(STORES.SETTINGS, toStore, SETTINGS_KEY);

    return this.get();
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
