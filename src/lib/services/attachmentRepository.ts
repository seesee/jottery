/**
 * Attachment repository implementation for IndexedDB blob storage
 */

import type { AttachmentRepository } from '../types';
import { getDB, STORES } from './db';

class IndexedDBAttachmentRepository implements AttachmentRepository {
  /**
   * Store attachment blob data
   */
  async storeBlob(id: string, data: ArrayBuffer): Promise<void> {
    const db = getDB();
    await db.put(STORES.ATTACHMENTS, data, id);
  }

  /**
   * Retrieve attachment blob data
   */
  async getBlob(id: string): Promise<ArrayBuffer | null> {
    const db = getDB();
    const data = await db.get(STORES.ATTACHMENTS, id);
    return data || null;
  }

  /**
   * Delete attachment blob
   */
  async deleteBlob(id: string): Promise<void> {
    const db = getDB();
    await db.delete(STORES.ATTACHMENTS, id);
  }

  /**
   * Store thumbnail blob
   */
  async storeThumbnail(id: string, data: ArrayBuffer): Promise<void> {
    const db = getDB();
    await db.put(STORES.THUMBNAILS, data, id);
  }

  /**
   * Retrieve thumbnail blob
   */
  async getThumbnail(id: string): Promise<ArrayBuffer | null> {
    const db = getDB();
    const data = await db.get(STORES.THUMBNAILS, id);
    return data || null;
  }

  /**
   * Delete thumbnail blob
   */
  async deleteThumbnail(id: string): Promise<void> {
    const db = getDB();
    await db.delete(STORES.THUMBNAILS, id);
  }

  /**
   * Delete both attachment and thumbnail
   */
  async deleteAll(id: string): Promise<void> {
    await Promise.all([this.deleteBlob(id), this.deleteThumbnail(id)]);
  }

  /**
   * Get total size of all attachments (approximate)
   */
  async getTotalSize(): Promise<number> {
    const db = getDB();
    const tx = db.transaction(STORES.ATTACHMENTS, 'readonly');
    let cursor = await tx.store.openCursor();

    let totalSize = 0;
    while (cursor) {
      if (cursor.value) {
        totalSize += cursor.value.byteLength;
      }
      cursor = await cursor.continue();
    }

    await tx.done;
    return totalSize;
  }

  /**
   * List all attachment IDs
   */
  async listAllIds(): Promise<string[]> {
    const db = getDB();
    const keys = await db.getAllKeys(STORES.ATTACHMENTS);
    return keys as string[]; // Keys are strings in our schema
  }
}

/**
 * Singleton instance
 */
export const attachmentRepository = new IndexedDBAttachmentRepository();
