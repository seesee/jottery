/**
 * Tests for conflictService
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { initTestDB, cleanupTestDB, createTestNote } from '../../test/db-utils';
import { raiseConflictWithStoredNote, storeConflict, getConflicts, getConflictInfo, resolveKeepMine, resolveMerge, resolveKeepServer, resolveKeepBoth } from './conflictService';
import { noteRepository } from './noteRepository';
import { noteService } from './noteService';
import { syncRepository } from './syncRepository';
import { keyManager } from './keyManager';
import { cryptoService, decryptStringArray } from './crypto';
import { CryptoError } from '../errors';
import { attachmentRepository } from './attachmentRepository';
import { storageToSync } from './tagConversionService';
import * as vaultHealthService from './vaultHealthService';
import type { Attachment, ConflictData, Note } from '../types';

describe('conflictService', () => {
  let masterKey: CryptoKey;

  beforeEach(async () => {
    const { masterKey: key } = await initTestDB();
    masterKey = key;
    // Mock keyManager to return our test key
    vi.spyOn(keyManager, 'getMasterKey').mockReturnValue({ key: masterKey, keyBytes: new Uint8Array(32), derivedAt: Date.now() });
  });

  afterEach(async () => {
    vi.restoreAllMocks();
    await cleanupTestDB();
  });

  const createTestConflictData = async (noteId: string): Promise<ConflictData> => {
    // Create encrypted content for testing
    const serverContent = 'Server version of the note';
    const encryptedContent = await cryptoService.encryptText(serverContent, masterKey);

    return {
      noteId,
      serverContent: JSON.stringify(encryptedContent),
      serverTags: [],
      serverModifiedAt: new Date().toISOString(),
      serverVersion: 2,
      serverAttachments: [],
      serverPinned: false,
      serverSyntaxLanguage: 'plain',
      serverWordWrap: true,
      detectedAt: new Date().toISOString(),
    };
  };

  describe('storeConflict', () => {
    it('should store conflict data for a note', async () => {
      const note = createTestNote({ content: 'Local content' });
      await noteRepository.create(note);

      await storeConflict(note.id, {
        serverContent: 'encrypted-server-content',
        serverTags: ['tag1'],
        serverModifiedAt: new Date().toISOString(),
        serverVersion: 2,
        serverAttachments: [],
        serverPinned: true,
        serverSyntaxLanguage: 'markdown',
        serverWordWrap: false,
      });

      const syncMeta = await syncRepository.getNoteSyncMetadata(note.id);
      expect(syncMeta).not.toBeNull();
      expect(syncMeta?.lastSyncStatus).toBe('conflict');
      expect(syncMeta?.conflictData).toBeDefined();
      expect(syncMeta?.conflictData?.serverContent).toBe('encrypted-server-content');
      expect(syncMeta?.conflictData?.serverPinned).toBe(true);
    });

    it('should set error message when storing conflict', async () => {
      const note = createTestNote();
      await noteRepository.create(note);

      await storeConflict(note.id, {
        serverContent: 'content',
        serverTags: [],
        serverModifiedAt: new Date().toISOString(),
        serverVersion: 1,
        serverAttachments: [],
        serverPinned: false,
      });

      const syncMeta = await syncRepository.getNoteSyncMetadata(note.id);
      expect(syncMeta?.errorMessage).toContain('manual resolution required');
    });

    it('should store hash chain fields when provided', async () => {
      const note = createTestNote({ id: 'note-hash' });
      await noteRepository.create(note);

      await storeConflict(note.id, {
        serverContent: 'encrypted-content',
        serverTags: ['tag1'],
        serverModifiedAt: new Date().toISOString(),
        serverVersion: 2,
        serverAttachments: [],
        serverPinned: false,
        // Hash chain fields
        serverContentHash: 'hash-abc123',
        serverParentHash: 'hash-parent',
        serverHashChain: ['hash-abc123', 'hash-parent', 'hash-grandparent'],
      });

      const syncMeta = await syncRepository.getNoteSyncMetadata(note.id);
      expect(syncMeta?.conflictData?.serverContentHash).toBe('hash-abc123');
      expect(syncMeta?.conflictData?.serverParentHash).toBe('hash-parent');
      expect(syncMeta?.conflictData?.serverHashChain).toEqual(['hash-abc123', 'hash-parent', 'hash-grandparent']);
    });

    it('should store ancestor data for 3-way merge when provided', async () => {
      const note = createTestNote({ id: 'note-ancestor' });
      await noteRepository.create(note);

      // Create encrypted ancestor content
      const ancestorContent = 'Original content before divergence';
      const encryptedAncestor = await cryptoService.encryptText(ancestorContent, masterKey);

      await storeConflict(note.id, {
        serverContent: 'encrypted-server-content',
        serverTags: [],
        serverModifiedAt: new Date().toISOString(),
        serverVersion: 2,
        serverAttachments: [],
        serverPinned: false,
        // Ancestor data
        ancestorHash: 'hash-common-ancestor',
        ancestorContent: JSON.stringify(encryptedAncestor),
        ancestorTags: [],
      });

      const syncMeta = await syncRepository.getNoteSyncMetadata(note.id);
      expect(syncMeta?.conflictData?.ancestorHash).toBe('hash-common-ancestor');
      expect(syncMeta?.conflictData?.ancestorContent).toBe(JSON.stringify(encryptedAncestor));
    });
  });

  describe('getConflicts', () => {
    it('should return empty array when no conflicts exist', async () => {
      const conflicts = await getConflicts();
      expect(conflicts).toEqual([]);
    });

    it('should return note IDs with conflict status', async () => {
      const note1 = createTestNote({ id: 'note-1' });
      const note2 = createTestNote({ id: 'note-2' });
      await noteRepository.create(note1);
      await noteRepository.create(note2);

      // Set note1 as conflict
      await syncRepository.updateNoteSyncMetadata('note-1', {
        noteId: 'note-1',
        lastSyncStatus: 'conflict',
        conflictData: await createTestConflictData('note-1'),
      });

      // Set note2 as synced (no conflict)
      await syncRepository.updateNoteSyncMetadata('note-2', {
        noteId: 'note-2',
        lastSyncStatus: 'synced',
      });

      const conflicts = await getConflicts();
      expect(conflicts).toHaveLength(1);
      expect(conflicts).toContain('note-1');
    });

    it('should not include notes without conflictData', async () => {
      const note = createTestNote({ id: 'note-1' });
      await noteRepository.create(note);

      // Set conflict status but no conflictData
      await syncRepository.updateNoteSyncMetadata('note-1', {
        noteId: 'note-1',
        lastSyncStatus: 'conflict',
        // No conflictData
      });

      const conflicts = await getConflicts();
      expect(conflicts).toHaveLength(0);
    });
  });

  describe('getConflictInfo', () => {
    it('should return null if note does not exist', async () => {
      const info = await getConflictInfo('non-existent');
      expect(info).toBeNull();
    });

    it('should return null if no conflict data exists', async () => {
      const note = createTestNote({ id: 'note-1' });
      await noteRepository.create(note);

      const info = await getConflictInfo('note-1');
      expect(info).toBeNull();
    });

    it('should return conflict info with decrypted content', async () => {
      const note = createTestNote({ id: 'note-1', content: 'Local content' });
      await noteRepository.create(note);

      const conflictData = await createTestConflictData('note-1');
      await syncRepository.updateNoteSyncMetadata('note-1', {
        noteId: 'note-1',
        lastSyncStatus: 'conflict',
        conflictData,
      });

      const info = await getConflictInfo('note-1');
      expect(info).not.toBeNull();
      expect(info?.noteId).toBe('note-1');
      expect(info?.localNote.content).toBe('Local content');
      expect(info?.serverContent).toBe('Server version of the note');
    });

    it('should throw if application is locked', async () => {
      vi.spyOn(keyManager, 'getMasterKey').mockReturnValue(null);

      await expect(getConflictInfo('note-1')).rejects.toThrow('Application is locked');
    });

    it('should return hash chain fields in conflict info', async () => {
      const note = createTestNote({ id: 'note-hash-info' });
      await noteRepository.create(note);

      const conflictData = await createTestConflictData('note-hash-info');
      // Add hash chain fields
      conflictData.serverContentHash = 'hash-server';
      conflictData.serverParentHash = 'hash-server-parent';

      await syncRepository.updateNoteSyncMetadata('note-hash-info', {
        noteId: 'note-hash-info',
        lastSyncStatus: 'conflict',
        conflictData,
      });

      const info = await getConflictInfo('note-hash-info');
      expect(info?.serverContentHash).toBe('hash-server');
      expect(info?.serverParentHash).toBe('hash-server-parent');
    });

    it('should return decrypted ancestor content when available', async () => {
      const note = createTestNote({ id: 'note-ancestor-info' });
      await noteRepository.create(note);

      // Create encrypted ancestor content
      const ancestorContent = 'Common ancestor content';
      const encryptedAncestor = await cryptoService.encryptText(ancestorContent, masterKey);

      const conflictData = await createTestConflictData('note-ancestor-info');
      conflictData.ancestorHash = 'hash-common';
      conflictData.ancestorContent = JSON.stringify(encryptedAncestor);
      conflictData.ancestorTags = [];

      await syncRepository.updateNoteSyncMetadata('note-ancestor-info', {
        noteId: 'note-ancestor-info',
        lastSyncStatus: 'conflict',
        conflictData,
      });

      const info = await getConflictInfo('note-ancestor-info');
      expect(info?.ancestorHash).toBe('hash-common');
      expect(info?.ancestorContent).toBe('Common ancestor content');
    });

    it('should handle missing ancestor data gracefully', async () => {
      const note = createTestNote({ id: 'note-no-ancestor' });
      await noteRepository.create(note);

      const conflictData = await createTestConflictData('note-no-ancestor');
      // No ancestor data set

      await syncRepository.updateNoteSyncMetadata('note-no-ancestor', {
        noteId: 'note-no-ancestor',
        lastSyncStatus: 'conflict',
        conflictData,
      });

      const info = await getConflictInfo('note-no-ancestor');
      expect(info?.ancestorHash).toBeUndefined();
      expect(info?.ancestorContent).toBeUndefined();
      expect(info?.ancestorTags).toBeUndefined();
    });
  });

  describe('resolveKeepMine', () => {
    it('should clear conflict and mark note for sync', async () => {
      const note = createTestNote({ id: 'note-1' });
      await noteRepository.create(note);

      await syncRepository.updateNoteSyncMetadata('note-1', {
        noteId: 'note-1',
        lastSyncStatus: 'conflict',
        conflictData: await createTestConflictData('note-1'),
      });

      await resolveKeepMine('note-1');

      const syncMeta = await syncRepository.getNoteSyncMetadata('note-1');
      expect(syncMeta?.lastSyncStatus).toBe('pending');
      expect(syncMeta?.conflictData).toBeUndefined();
    });

    it('should update note modifiedAt timestamp', async () => {
      const oldDate = '2020-01-01T00:00:00.000Z';
      const note = createTestNote({ id: 'note-1', modifiedAt: oldDate });
      await noteRepository.create(note);

      await syncRepository.updateNoteSyncMetadata('note-1', {
        noteId: 'note-1',
        lastSyncStatus: 'conflict',
        conflictData: await createTestConflictData('note-1'),
      });

      await resolveKeepMine('note-1');

      const updatedNote = await noteRepository.getById('note-1');
      expect(new Date(updatedNote!.modifiedAt).getTime()).toBeGreaterThan(new Date(oldDate).getTime());
    });

    it('should throw if note does not exist', async () => {
      await expect(resolveKeepMine('non-existent')).rejects.toThrow('not found');
    });
  });

  describe('resolveKeepServer', () => {
    it('should update note with server content', async () => {
      const note = createTestNote({ id: 'note-1', content: 'Local content', pinned: false });
      await noteRepository.create(note);

      const conflictData = await createTestConflictData('note-1');
      await syncRepository.updateNoteSyncMetadata('note-1', {
        noteId: 'note-1',
        lastSyncStatus: 'conflict',
        conflictData,
      });

      await resolveKeepServer('note-1');

      const updatedNote = await noteRepository.getById('note-1');
      // Note: Content will be encrypted server content
      expect(updatedNote?.content).toBe(conflictData.serverContent);
    });

    it('should clear conflict and mark as synced', async () => {
      const note = createTestNote({ id: 'note-1' });
      await noteRepository.create(note);

      await syncRepository.updateNoteSyncMetadata('note-1', {
        noteId: 'note-1',
        lastSyncStatus: 'conflict',
        conflictData: await createTestConflictData('note-1'),
      });

      await resolveKeepServer('note-1');

      const syncMeta = await syncRepository.getNoteSyncMetadata('note-1');
      expect(syncMeta?.lastSyncStatus).toBe('synced');
      expect(syncMeta?.conflictData).toBeUndefined();
    });

    it('should throw if no conflict data exists', async () => {
      const note = createTestNote({ id: 'note-1' });
      await noteRepository.create(note);

      await expect(resolveKeepServer('note-1')).rejects.toThrow('not found');
    });

    it('should throw if application is locked', async () => {
      vi.spyOn(keyManager, 'getMasterKey').mockReturnValue(null);

      await expect(resolveKeepServer('note-1')).rejects.toThrow('Application is locked');
    });
  });

  describe('resolveKeepBoth', () => {
    it('should create new note from server version', async () => {
      const note = createTestNote({ id: 'note-1', content: 'Local content' });
      await noteRepository.create(note);

      const conflictData = await createTestConflictData('note-1');
      await syncRepository.updateNoteSyncMetadata('note-1', {
        noteId: 'note-1',
        lastSyncStatus: 'conflict',
        conflictData,
      });

      const newNoteId = await resolveKeepBoth('note-1');

      expect(newNoteId).toBeDefined();
      expect(newNoteId).not.toBe('note-1');

      // Original note should still exist
      const originalNote = await noteRepository.getById('note-1');
      expect(originalNote).not.toBeNull();

      // New note should exist
      const newNote = await noteRepository.getById(newNoteId);
      expect(newNote).not.toBeNull();
    });

    it('should add conflict-copy tag to new note', async () => {
      const note = createTestNote({ id: 'note-1' });
      await noteRepository.create(note);

      const conflictData = await createTestConflictData('note-1');
      await syncRepository.updateNoteSyncMetadata('note-1', {
        noteId: 'note-1',
        lastSyncStatus: 'conflict',
        conflictData,
      });

      const newNoteId = await resolveKeepBoth('note-1');

      const newNote = await noteRepository.getById(newNoteId);
      expect(newNote).not.toBeNull();

      // Decrypt tags to verify the conflict-copy tag was added
      const encryptedTags = JSON.parse(newNote!.tags[0]);
      const decryptedTags = await decryptStringArray(encryptedTags, masterKey);
      expect(decryptedTags).toContain('conflict-copy');
    });

    it('should clear conflict on original note', async () => {
      const note = createTestNote({ id: 'note-1' });
      await noteRepository.create(note);

      await syncRepository.updateNoteSyncMetadata('note-1', {
        noteId: 'note-1',
        lastSyncStatus: 'conflict',
        conflictData: await createTestConflictData('note-1'),
      });

      await resolveKeepBoth('note-1');

      const syncMeta = await syncRepository.getNoteSyncMetadata('note-1');
      expect(syncMeta?.lastSyncStatus).toBe('pending');
      expect(syncMeta?.conflictData).toBeUndefined();
    });

    it('should throw if no conflict data exists', async () => {
      const note = createTestNote({ id: 'note-1' });
      await noteRepository.create(note);

      await expect(resolveKeepBoth('note-1')).rejects.toThrow('not found');
    });
  });

  describe('Integration Scenarios', () => {
    it('should handle full conflict detection and resolution workflow', async () => {
      // Create note
      const note = createTestNote({ id: 'note-1', content: 'Original content' });
      await noteRepository.create(note);

      // Simulate conflict detected during sync
      await storeConflict('note-1', {
        serverContent: 'encrypted-server-content',
        serverTags: [],
        serverModifiedAt: new Date().toISOString(),
        serverVersion: 2,
        serverAttachments: [],
        serverPinned: false,
      });

      // Verify conflict exists
      let conflicts = await getConflicts();
      expect(conflicts).toContain('note-1');

      // Resolve conflict
      await resolveKeepMine('note-1');

      // Verify conflict is resolved
      conflicts = await getConflicts();
      expect(conflicts).not.toContain('note-1');

      const syncMeta = await syncRepository.getNoteSyncMetadata('note-1');
      expect(syncMeta?.lastSyncStatus).toBe('pending');
    });

    it('should handle multiple concurrent conflicts', async () => {
      // Create multiple notes
      const note1 = createTestNote({ id: 'note-1' });
      const note2 = createTestNote({ id: 'note-2' });
      const note3 = createTestNote({ id: 'note-3' });
      await noteRepository.create(note1);
      await noteRepository.create(note2);
      await noteRepository.create(note3);

      // Set conflicts on all
      for (const noteId of ['note-1', 'note-2', 'note-3']) {
        await storeConflict(noteId, {
          serverContent: JSON.stringify(await cryptoService.encryptText(`server-${noteId}`, masterKey)),
          serverTags: [],
          serverModifiedAt: new Date().toISOString(),
          serverVersion: 2,
          serverAttachments: [],
          serverPinned: false,
        });
      }

      // Verify all conflicts exist
      let conflicts = await getConflicts();
      expect(conflicts).toHaveLength(3);

      // Resolve conflicts with different strategies
      await resolveKeepMine('note-1');
      await resolveKeepServer('note-2');
      await resolveKeepBoth('note-3');

      // Verify all resolved
      conflicts = await getConflicts();
      expect(conflicts).toHaveLength(0);
    });
  });
});

describe('a server copy that fails to decrypt is never treated as plaintext', () => {
  const FORGED = '<img src=x onerror=alert(1)>';
  let masterKey: CryptoKey;
  let noteId: string;

  beforeEach(async () => {
    const testDB = await initTestDB();
    masterKey = testDB.masterKey;
    keyManager.setMasterKey({ key: masterKey, keyBytes: new Uint8Array(32), derivedAt: Date.now() });

    const content = JSON.stringify(await cryptoService.encryptText('Mine', masterKey));
    const note = createTestNote({ content });
    await noteRepository.create(note);
    noteId = note.id;
  });

  afterEach(async () => {
    keyManager.clearMasterKey();
    await cleanupTestDB();
  });

  async function conflictWith(serverContent: string) {
    await storeConflict(noteId, {
      serverContent,
      serverTags: [],
      serverModifiedAt: new Date().toISOString(),
      serverVersion: 2,
      serverAttachments: [],
      serverPinned: false,
      serverSyntaxLanguage: 'html',
    });
  }

  async function forgedUnderAnotherKey(): Promise<string> {
    const other = await crypto.subtle.generateKey({ name: 'AES-GCM', length: 256 }, true, ['encrypt', 'decrypt']);
    return JSON.stringify(await cryptoService.encryptText('forged', other));
  }

  it('reports forged text as unreadable rather than showing it', async () => {
    await conflictWith(FORGED);
    const info = await getConflictInfo(noteId);
    expect(info).not.toBeNull();
    expect(info?.serverContent).toBeNull();
  });

  it('reports a failed authentication check as unreadable', async () => {
    await conflictWith(await forgedUnderAnotherKey());
    expect((await getConflictInfo(noteId))?.serverContent).toBeNull();
  });

  it('still shows a server copy that does decrypt', async () => {
    await conflictWith(JSON.stringify(await cryptoService.encryptText('Theirs', masterKey)));
    expect((await getConflictInfo(noteId))?.serverContent).toBe('Theirs');
  });

  it('refuses Keep Both, and creates no note from forged text', async () => {
    await conflictWith(FORGED);
    const before = (await noteRepository.getAll()).length;

    await expect(resolveKeepBoth(noteId)).rejects.toBeInstanceOf(CryptoError);

    expect((await noteRepository.getAll()).length).toBe(before);
    expect((await syncRepository.getNoteSyncMetadata(noteId))?.conflictData).toBeDefined();
  });

  it('refuses Keep Server, and leaves the local note alone', async () => {
    await conflictWith(FORGED);
    const before = await noteRepository.getById(noteId);

    await expect(resolveKeepServer(noteId)).rejects.toBeInstanceOf(CryptoError);

    expect((await noteRepository.getById(noteId))?.content).toBe(before?.content);
  });

  it('still allows Keep Mine', async () => {
    await conflictWith(FORGED);
    await expect(resolveKeepMine(noteId)).resolves.toBeUndefined();
    expect((await syncRepository.getNoteSyncMetadata(noteId))?.conflictData).toBeUndefined();
  });
});

describe('a pull that lands while the editor has unsaved text', () => {
  let masterKey: CryptoKey;
  let noteId: string;

  beforeEach(async () => {
    masterKey = (await initTestDB()).masterKey;
    keyManager.setMasterKey({ key: masterKey, keyBytes: new Uint8Array(32), derivedAt: Date.now() });

    const pulled = await noteService.createNote('Pulled text', ['from-elsewhere']);
    noteId = pulled.id;
  });

  afterEach(async () => {
    keyManager.clearMasterKey();
    await cleanupTestDB();
  });

  it('records the pulled copy, so it can be chosen', async () => {
    await raiseConflictWithStoredNote(noteId);

    const info = await getConflictInfo(noteId);
    expect(info?.serverContent).toBe('Pulled text');
    expect(info?.serverTags).toEqual(['from-elsewhere']);
    expect(await getConflicts()).toEqual([noteId]);
  });

  it('keeps the pulled copy after the editor saves over it', async () => {
    await raiseConflictWithStoredNote(noteId);

    await noteService.updateNote(noteId, { content: 'Typed here' });

    expect((await getConflictInfo(noteId))?.serverContent).toBe('Pulled text');
  });

  it('can bring the pulled copy back', async () => {
    await raiseConflictWithStoredNote(noteId);
    await noteService.updateNote(noteId, { content: 'Typed here' });

    await resolveKeepServer(noteId);

    const restored = await noteService.getNote(noteId);
    expect(restored?.content).toBe('Pulled text');
    expect(restored?.tags).toEqual(['from-elsewhere']);
  });

  it('can keep what was typed', async () => {
    await raiseConflictWithStoredNote(noteId);
    await noteService.updateNote(noteId, { content: 'Typed here' });

    await resolveKeepMine(noteId);

    expect((await noteService.getNote(noteId))?.content).toBe('Typed here');
    expect(await getConflicts()).toEqual([]);
  });

  it('does nothing for a note that has gone', async () => {
    await expect(raiseConflictWithStoredNote('a0000000-0000-4000-8000-000000000099')).resolves.toBeUndefined();
  });
});

describe('what a resolved conflict does to tags and attachments', () => {
  let masterKey: CryptoKey;
  const noteId = 'a0000000-0000-4000-8000-000000000040';
  const attachmentId = 'b0000000-0000-4000-8000-000000000040';
  const reference = { id: attachmentId, filename: 'name', mimeType: 'image/png', size: 3, data: attachmentId };

  beforeEach(async () => {
    masterKey = (await initTestDB()).masterKey;
    keyManager.setMasterKey({ key: masterKey, keyBytes: new Uint8Array(32), derivedAt: Date.now() });
  });

  afterEach(async () => {
    keyManager.clearMasterKey();
    vi.restoreAllMocks();
    await cleanupTestDB();
  });

  const encrypted = async (text: string) => JSON.stringify(await cryptoService.encryptText(text, masterKey));
  const syncTag = async (tag: string) => encrypted(JSON.stringify(tag));

  async function conflictOver(serverTags: string[], serverAttachments: Attachment[] = []): Promise<void> {
    await noteRepository.create(createTestNote({
      id: noteId, content: await encrypted('mine'), tags: [], needsSync: false,
      contentHash: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa=', hashChain: ['aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa='],
    }));
    await storeConflict(noteId, {
      serverContent: await encrypted('theirs'), serverTags, serverModifiedAt: '2026-11-05T01:02:03.000Z',
      serverVersion: 2, serverAttachments, serverPinned: false,
      serverContentHash: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb=', serverParentHash: null,
      serverHashChain: ['bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb='],
    });
  }

  const pushedTagsOf = async (note: Note) => {
    const pushed = await storageToSync(note.tags, masterKey);
    return Promise.all(pushed.map(async tag => JSON.parse(await cryptoService.decryptText(JSON.parse(tag), masterKey))));
  };

  describe('Keep Server', () => {
    it('keeps a note\'s only tag when it is next pushed', async () => {
      // One tag looks like the storage format, which is how it used to vanish
      await conflictOver([await syncTag('work')]);

      await resolveKeepServer(noteId);

      expect(await pushedTagsOf(await noteRepository.getById(noteId) as Note)).toEqual(['work']);
    });

    it('keeps several tags', async () => {
      await conflictOver([await syncTag('a'), await syncTag('b')]);

      await resolveKeepServer(noteId);

      const note = await noteRepository.getById(noteId) as Note;
      expect(await pushedTagsOf(note)).toEqual(['a', 'b']);
      expect(note.tags).toHaveLength(1); // Stored as one blob, as every stored note is
    });

    it('does not mark the note as changed, because it now matches the server', async () => {
      await conflictOver([]);

      await resolveKeepServer(noteId);

      const note = await noteRepository.getById(noteId) as Note;
      expect(note.needsSync).toBe(false);
      expect(note.modifiedAt).toBe('2026-11-05T01:02:03.000Z');
    });
  });

  describe('Keep Mine', () => {
    it('keeps what the server added, which an authoritative push would otherwise remove', async () => {
      await conflictOver([], [reference]);

      await resolveKeepMine(noteId);

      expect((await noteRepository.getById(noteId) as Note).attachments.map(a => a.id)).toEqual([attachmentId]);
    });
  });

  describe('Merge', () => {
    it('keeps what the server added too', async () => {
      await conflictOver([], [reference]);

      await resolveMerge(noteId, 'merged', []);

      expect((await noteRepository.getById(noteId) as Note).attachments.map(a => a.id)).toEqual([attachmentId]);
    });
  });

  describe('Keep Both', () => {
    const copyOf = async (id: string) => noteRepository.getById(id) as Promise<Note>;

    it('copies the server\'s attachments to the new note, under ids of their own', async () => {
      await conflictOver([], [reference]);
      await attachmentRepository.storeBlob(attachmentId, new Uint8Array([1, 2, 3]).buffer);

      const newId = await resolveKeepBoth(noteId);

      const [copied] = (await copyOf(newId)).attachments;
      expect(copied.id).not.toBe(attachmentId);
      expect(copied.data).toBe(copied.id);
      expect(copied.filename).toBe('name');
      expect(new Uint8Array((await attachmentRepository.getBlob(copied.data)) as ArrayBuffer)).toEqual(new Uint8Array([1, 2, 3]));
    });

    it('records the attachments in the copy\'s hash, so it syncs as a change', async () => {
      await conflictOver([], [reference]);
      await attachmentRepository.storeBlob(attachmentId, new Uint8Array([1]).buffer);

      const copy = await copyOf(await resolveKeepBoth(noteId));

      expect(copy.contentHash).toBe(await cryptoService.computeContentHash(copy));
      expect(copy.needsSync).toBe(true);
    });

    it('fetches a blob this device has never had', async () => {
      await conflictOver([], [reference]);
      vi.spyOn(vaultHealthService, 'repairAttachment').mockImplementation(async id => {
        await attachmentRepository.storeBlob(id, new Uint8Array([9]).buffer);
        return 'repaired';
      });

      const newId = await resolveKeepBoth(noteId);

      const [copied] = (await copyOf(newId)).attachments;
      expect(new Uint8Array((await attachmentRepository.getBlob(copied.data)) as ArrayBuffer)).toEqual(new Uint8Array([9]));
    });

    it('stops without changing anything if a blob cannot be fetched, so the option meant to lose nothing does not', async () => {
      await conflictOver([], [reference]);
      vi.spyOn(vaultHealthService, 'repairAttachment').mockResolvedValue('failed');
      const before = (await noteRepository.getAll()).length;

      await expect(resolveKeepBoth(noteId)).rejects.toThrow();

      expect((await noteRepository.getAll()).length).toBe(before);
      expect((await syncRepository.getNoteSyncMetadata(noteId))?.lastSyncStatus).toBe('conflict');
    });
  });
});
