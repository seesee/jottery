/**
 * Encryption metadata repository implementation for IndexedDB
 */

import type { EncryptionRepository, EncryptionMetadata } from '../types';
import { getDB, STORES } from './db';

const METADATA_KEY = 'metadata';

class IndexedDBEncryptionRepository implements EncryptionRepository {
  /**
   * Get encryption metadata
   */
  async getMetadata(): Promise<EncryptionMetadata | null> {
    const db = getDB();
    const metadata = await db.get(STORES.ENCRYPTION, METADATA_KEY);
    return metadata || null;
  }

  /**
   * Store encryption metadata
   */
  async setMetadata(metadata: EncryptionMetadata): Promise<void> {
    const db = getDB();
    await db.put(STORES.ENCRYPTION, metadata, METADATA_KEY);
  }

  /**
   * Check if encryption is initialized
   */
  async isInitialized(): Promise<boolean> {
    const metadata = await this.getMetadata();
    return metadata !== null;
  }

  /**
   * Delete encryption metadata (caution: will make data unrecoverable)
   */
  async deleteMetadata(): Promise<void> {
    const db = getDB();
    await db.delete(STORES.ENCRYPTION, METADATA_KEY);
  }
}

/**
 * Singleton instance
 */
export const encryptionRepository = new IndexedDBEncryptionRepository();
