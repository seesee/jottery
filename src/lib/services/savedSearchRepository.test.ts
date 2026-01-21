import { describe, test, expect, beforeEach, afterEach } from 'vitest';
import * as savedSearchRepository from './savedSearchRepository';
import type { SavedSearch } from '../types/models';
import { initDB, getDB, STORES } from './db';

describe('savedSearchRepository', () => {
  // Initialize and clear the database before each test
  beforeEach(async () => {
    await initDB();
    const db = await getDB();
    const tx = db.transaction(STORES.SAVED_SEARCHES, 'readwrite');
    await tx.store.clear();
    await tx.done;
  });

  describe('create', () => {
    test('should create a saved search with all required fields', async () => {
      const search = await savedSearchRepository.create('My Search', 'color:red');

      expect(search).toBeDefined();
      expect(search.id).toBeTruthy();
      expect(search.name).toBe('My Search');
      expect(search.query).toBe('color:red');
      expect(search.order).toBe(1);
      expect(search.createdAt).toBeTruthy();
      expect(search.modifiedAt).toBeTruthy();
      expect(search.deleted).toBe(false);
      expect(search.version).toBe(1);
      expect(search.needsSync).toBe(true);
    });

    test('should assign incrementing order values', async () => {
      const search1 = await savedSearchRepository.create('Search 1', 'query1');
      const search2 = await savedSearchRepository.create('Search 2', 'query2');
      const search3 = await savedSearchRepository.create('Search 3', 'query3');

      expect(search1.order).toBe(1);
      expect(search2.order).toBe(2);
      expect(search3.order).toBe(3);
    });

    test('should create valid ISO 8601 timestamps', async () => {
      const search = await savedSearchRepository.create('Test', 'test query');

      const createdDate = new Date(search.createdAt);
      const modifiedDate = new Date(search.modifiedAt);

      expect(createdDate.toISOString()).toBe(search.createdAt);
      expect(modifiedDate.toISOString()).toBe(search.modifiedAt);
      expect(search.createdAt).toBe(search.modifiedAt);
    });

    test('should generate unique IDs for each search', async () => {
      const search1 = await savedSearchRepository.create('Search 1', 'query1');
      const search2 = await savedSearchRepository.create('Search 2', 'query2');

      expect(search1.id).not.toBe(search2.id);
    });

    test('should persist to database', async () => {
      const created = await savedSearchRepository.create('Persistent', 'query');
      const retrieved = await savedSearchRepository.getById(created.id);

      expect(retrieved).toBeDefined();
      expect(retrieved?.id).toBe(created.id);
      expect(retrieved?.name).toBe('Persistent');
    });
  });

  describe('getAll', () => {
    test('should return empty array when no searches exist', async () => {
      const searches = await savedSearchRepository.getAll();
      expect(searches).toEqual([]);
    });

    test('should return all non-deleted searches', async () => {
      await savedSearchRepository.create('Search 1', 'query1');
      await savedSearchRepository.create('Search 2', 'query2');
      await savedSearchRepository.create('Search 3', 'query3');

      const searches = await savedSearchRepository.getAll();
      expect(searches.length).toBe(3);
    });

    test('should not return deleted searches', async () => {
      const search1 = await savedSearchRepository.create('Search 1', 'query1');
      const search2 = await savedSearchRepository.create('Search 2', 'query2');
      await savedSearchRepository.softDelete(search1.id);

      const searches = await savedSearchRepository.getAll();
      expect(searches.length).toBe(1);
      expect(searches[0].id).toBe(search2.id);
    });

    test('should return searches ordered by order field', async () => {
      const search1 = await savedSearchRepository.create('Search 1', 'query1');
      const search2 = await savedSearchRepository.create('Search 2', 'query2');
      const search3 = await savedSearchRepository.create('Search 3', 'query3');

      const searches = await savedSearchRepository.getAll();
      expect(searches[0].order).toBeLessThanOrEqual(searches[1].order);
      expect(searches[1].order).toBeLessThanOrEqual(searches[2].order);
    });
  });

  describe('getById', () => {
    test('should return undefined for non-existent ID', async () => {
      const search = await savedSearchRepository.getById('non-existent-id');
      expect(search).toBeUndefined();
    });

    test('should retrieve saved search by ID', async () => {
      const created = await savedSearchRepository.create('Test Search', 'test query');
      const retrieved = await savedSearchRepository.getById(created.id);

      expect(retrieved).toBeDefined();
      expect(retrieved?.id).toBe(created.id);
      expect(retrieved?.name).toBe('Test Search');
      expect(retrieved?.query).toBe('test query');
    });

    test('should retrieve deleted searches by ID', async () => {
      const created = await savedSearchRepository.create('Test', 'query');
      await savedSearchRepository.softDelete(created.id);

      const retrieved = await savedSearchRepository.getById(created.id);
      expect(retrieved).toBeDefined();
      expect(retrieved?.deleted).toBe(true);
    });
  });

  describe('update', () => {
    test('should update name field', async () => {
      const search = await savedSearchRepository.create('Original Name', 'query');
      const updated = await savedSearchRepository.update(search.id, { name: 'New Name' });

      expect(updated.name).toBe('New Name');
      expect(updated.query).toBe('query'); // Unchanged
    });

    test('should update query field', async () => {
      const search = await savedSearchRepository.create('Name', 'original query');
      const updated = await savedSearchRepository.update(search.id, { query: 'new query' });

      expect(updated.query).toBe('new query');
      expect(updated.name).toBe('Name'); // Unchanged
    });

    test('should update order field', async () => {
      const search = await savedSearchRepository.create('Name', 'query');
      const updated = await savedSearchRepository.update(search.id, { order: 99 });

      expect(updated.order).toBe(99);
    });

    test('should increment version on update', async () => {
      const search = await savedSearchRepository.create('Name', 'query');
      expect(search.version).toBe(1);

      const updated1 = await savedSearchRepository.update(search.id, { name: 'New Name' });
      expect(updated1.version).toBe(2);

      const updated2 = await savedSearchRepository.update(search.id, { query: 'New Query' });
      expect(updated2.version).toBe(3);
    });

    test('should update modifiedAt timestamp', async () => {
      const search = await savedSearchRepository.create('Name', 'query');
      const originalModified = search.modifiedAt;

      // Wait a tiny bit to ensure timestamp difference
      await new Promise(resolve => setTimeout(resolve, 10));

      const updated = await savedSearchRepository.update(search.id, { name: 'New Name' });
      expect(updated.modifiedAt).not.toBe(originalModified);
      expect(new Date(updated.modifiedAt).getTime()).toBeGreaterThan(
        new Date(originalModified).getTime()
      );
    });

    test('should set needsSync flag on update', async () => {
      const search = await savedSearchRepository.create('Name', 'query');

      // Manually clear needsSync to simulate synced state
      const db = await getDB();
      await db.put(STORES.SAVED_SEARCHES, { ...search, needsSync: false });

      const updated = await savedSearchRepository.update(search.id, { name: 'New Name' });
      expect(updated.needsSync).toBe(true);
    });

    test('should throw error for non-existent ID', async () => {
      await expect(
        savedSearchRepository.update('non-existent', { name: 'New Name' })
      ).rejects.toThrow('SavedSearch not found');
    });

    test('should persist updates to database', async () => {
      const search = await savedSearchRepository.create('Original', 'query');
      await savedSearchRepository.update(search.id, { name: 'Updated' });

      const retrieved = await savedSearchRepository.getById(search.id);
      expect(retrieved?.name).toBe('Updated');
    });
  });

  describe('softDelete', () => {
    test('should mark search as deleted', async () => {
      const search = await savedSearchRepository.create('Test', 'query');
      await savedSearchRepository.softDelete(search.id);

      const retrieved = await savedSearchRepository.getById(search.id);
      expect(retrieved?.deleted).toBe(true);
    });

    test('should set deletedAt timestamp', async () => {
      const search = await savedSearchRepository.create('Test', 'query');
      await savedSearchRepository.softDelete(search.id);

      const retrieved = await savedSearchRepository.getById(search.id);
      expect(retrieved?.deletedAt).toBeTruthy();

      const deletedDate = new Date(retrieved!.deletedAt!);
      expect(deletedDate.toISOString()).toBe(retrieved!.deletedAt);
    });

    test('should increment version on delete', async () => {
      const search = await savedSearchRepository.create('Test', 'query');
      await savedSearchRepository.softDelete(search.id);

      const retrieved = await savedSearchRepository.getById(search.id);
      expect(retrieved?.version).toBe(2);
    });

    test('should update modifiedAt on delete', async () => {
      const search = await savedSearchRepository.create('Test', 'query');
      const originalModified = search.modifiedAt;

      await new Promise(resolve => setTimeout(resolve, 10));
      await savedSearchRepository.softDelete(search.id);

      const retrieved = await savedSearchRepository.getById(search.id);
      expect(retrieved?.modifiedAt).not.toBe(originalModified);
    });

    test('should set needsSync on delete', async () => {
      const search = await savedSearchRepository.create('Test', 'query');
      await savedSearchRepository.softDelete(search.id);

      const retrieved = await savedSearchRepository.getById(search.id);
      expect(retrieved?.needsSync).toBe(true);
    });
  });

  describe('reorder', () => {
    test('should update order values based on array position', async () => {
      const search1 = await savedSearchRepository.create('Search 1', 'query1');
      const search2 = await savedSearchRepository.create('Search 2', 'query2');
      const search3 = await savedSearchRepository.create('Search 3', 'query3');

      // Reorder: 3, 1, 2
      await savedSearchRepository.reorder([search3.id, search1.id, search2.id]);

      const retrieved1 = await savedSearchRepository.getById(search1.id);
      const retrieved2 = await savedSearchRepository.getById(search2.id);
      const retrieved3 = await savedSearchRepository.getById(search3.id);

      expect(retrieved3?.order).toBe(1);
      expect(retrieved1?.order).toBe(2);
      expect(retrieved2?.order).toBe(3);
    });

    test('should handle empty array', async () => {
      await expect(savedSearchRepository.reorder([])).resolves.not.toThrow();
    });

    test('should handle single item', async () => {
      const search = await savedSearchRepository.create('Search', 'query');
      await savedSearchRepository.reorder([search.id]);

      const retrieved = await savedSearchRepository.getById(search.id);
      expect(retrieved?.order).toBe(1);
    });

    test('should skip non-existent IDs gracefully', async () => {
      const search1 = await savedSearchRepository.create('Search 1', 'query1');
      const search2 = await savedSearchRepository.create('Search 2', 'query2');

      await savedSearchRepository.reorder([
        search1.id,
        'non-existent-id',
        search2.id
      ]);

      const retrieved1 = await savedSearchRepository.getById(search1.id);
      const retrieved2 = await savedSearchRepository.getById(search2.id);

      expect(retrieved1?.order).toBe(1);
      expect(retrieved2?.order).toBe(3); // Position 3 in array
    });
  });

  describe('getAllNeedingSync', () => {
    test('should return empty array when no searches need sync', async () => {
      // Create and manually mark as synced
      const search = await savedSearchRepository.create('Test', 'query');
      const db = await getDB();
      await db.put(STORES.SAVED_SEARCHES, { ...search, needsSync: false });

      const needingSync = await savedSearchRepository.getAllNeedingSync();
      expect(needingSync).toEqual([]);
    });

    test('should return newly created searches', async () => {
      await savedSearchRepository.create('Search 1', 'query1');
      await savedSearchRepository.create('Search 2', 'query2');

      const needingSync = await savedSearchRepository.getAllNeedingSync();
      expect(needingSync.length).toBe(2);
    });

    test('should return updated searches', async () => {
      const search = await savedSearchRepository.create('Test', 'query');

      // Mark as synced
      const db = await getDB();
      await db.put(STORES.SAVED_SEARCHES, { ...search, needsSync: false });

      // Update it
      await savedSearchRepository.update(search.id, { name: 'Updated' });

      const needingSync = await savedSearchRepository.getAllNeedingSync();
      expect(needingSync.length).toBe(1);
      expect(needingSync[0].id).toBe(search.id);
    });

    test('should return deleted searches', async () => {
      const search = await savedSearchRepository.create('Test', 'query');

      // Mark as synced
      const db = await getDB();
      await db.put(STORES.SAVED_SEARCHES, { ...search, needsSync: false });

      // Delete it
      await savedSearchRepository.softDelete(search.id);

      const needingSync = await savedSearchRepository.getAllNeedingSync();
      expect(needingSync.length).toBe(1);
      expect(needingSync[0].deleted).toBe(true);
    });

    test('should not return synced searches', async () => {
      const search1 = await savedSearchRepository.create('Unsynced', 'query1');
      const search2 = await savedSearchRepository.create('To Sync', 'query2');

      // Mark search2 as synced
      const db = await getDB();
      await db.put(STORES.SAVED_SEARCHES, { ...search2, needsSync: false });

      const needingSync = await savedSearchRepository.getAllNeedingSync();
      expect(needingSync.length).toBe(1);
      expect(needingSync[0].id).toBe(search1.id);
    });
  });
});
