/**
 * Tests for noteService
 * Focuses on UI state persistence and sync-related behaviour
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { noteService } from './noteService';
import { noteRepository } from './noteRepository';
import { keyManager } from './keyManager';
import { initTestDB, cleanupTestDB } from '../../test/db-utils';

describe('noteService', () => {
  let masterKey: CryptoKey;

  beforeEach(async () => {
    const testDB = await initTestDB();
    masterKey = testDB.masterKey;

    // Set up authenticated key manager
    keyManager.setMasterKey({
      key: masterKey,
      derivedAt: Date.now(),
    });
  });

  afterEach(async () => {
    keyManager.clearMasterKey();
    await cleanupTestDB();
  });

  describe('updateNote', () => {
    describe('UI state changes should update modifiedAt for sync', () => {
      it('should update modifiedAt when showPreview changes', async () => {
        // Create a note
        const note = await noteService.createNote('Test content', ['test']);
        const originalModifiedAt = note.modifiedAt;

        // Wait a bit to ensure time difference
        await new Promise(resolve => setTimeout(resolve, 10));

        // Update showPreview
        const updated = await noteService.updateNote(note.id, {
          content: 'Test content', // Same content
          tags: ['test'], // Same tags
          showPreview: true, // Changed
        });

        // modifiedAt should be updated
        expect(updated.modifiedAt).not.toBe(originalModifiedAt);
        expect(new Date(updated.modifiedAt).getTime()).toBeGreaterThan(
          new Date(originalModifiedAt).getTime()
        );

        // showPreview should be saved
        expect(updated.showPreview).toBe(true);
      });

      it('should update modifiedAt when wordWrap changes', async () => {
        const note = await noteService.createNote('Test content', ['test']);
        const originalModifiedAt = note.modifiedAt;

        await new Promise(resolve => setTimeout(resolve, 10));

        const updated = await noteService.updateNote(note.id, {
          content: 'Test content',
          tags: ['test'],
          wordWrap: false, // Changed from default true
        });

        expect(updated.modifiedAt).not.toBe(originalModifiedAt);
        expect(updated.wordWrap).toBe(false);
      });

      it('should update modifiedAt when syntaxLanguage changes', async () => {
        const note = await noteService.createNote('Test content', ['test']);
        const originalModifiedAt = note.modifiedAt;

        await new Promise(resolve => setTimeout(resolve, 10));

        const updated = await noteService.updateNote(note.id, {
          content: 'Test content',
          tags: ['test'],
          syntaxLanguage: 'javascript', // Changed from default markdown
        });

        expect(updated.modifiedAt).not.toBe(originalModifiedAt);
        expect(updated.syntaxLanguage).toBe('javascript');
      });

      it('should NOT update modifiedAt when UI state is unchanged', async () => {
        const note = await noteService.createNote('Test content', ['test'], {
          showPreview: true,
          wordWrap: false,
          syntaxLanguage: 'javascript',
        });
        const originalModifiedAt = note.modifiedAt;

        await new Promise(resolve => setTimeout(resolve, 10));

        // Update with same values
        const updated = await noteService.updateNote(note.id, {
          content: 'Test content',
          tags: ['test'],
          showPreview: true, // Same
          wordWrap: false, // Same
          syntaxLanguage: 'javascript', // Same
        });

        // modifiedAt should NOT be updated since nothing changed
        expect(updated.modifiedAt).toBe(originalModifiedAt);
      });
    });

    describe('needsSync flag', () => {
      it('should set needsSync when showPreview changes', async () => {
        const note = await noteService.createNote('Test content', ['test']);

        // Clear needsSync by simulating a sync
        const rawNote = await noteRepository.getById(note.id);
        if (rawNote) {
          rawNote.needsSync = false;
          await noteRepository.update(rawNote, false, true);
        }

        // Update showPreview
        await noteService.updateNote(note.id, {
          content: 'Test content',
          tags: ['test'],
          showPreview: true,
        });

        // Check needsSync is set
        const updatedNote = await noteRepository.getById(note.id);
        expect(updatedNote?.needsSync).toBe(true);
      });

      it('should set needsSync when wordWrap changes', async () => {
        const note = await noteService.createNote('Test content', ['test']);

        const rawNote = await noteRepository.getById(note.id);
        if (rawNote) {
          rawNote.needsSync = false;
          await noteRepository.update(rawNote, false, true);
        }

        await noteService.updateNote(note.id, {
          content: 'Test content',
          tags: ['test'],
          wordWrap: false,
        });

        const updatedNote = await noteRepository.getById(note.id);
        expect(updatedNote?.needsSync).toBe(true);
      });

      it('should set needsSync when syntaxLanguage changes', async () => {
        const note = await noteService.createNote('Test content', ['test']);

        const rawNote = await noteRepository.getById(note.id);
        if (rawNote) {
          rawNote.needsSync = false;
          await noteRepository.update(rawNote, false, true);
        }

        await noteService.updateNote(note.id, {
          content: 'Test content',
          tags: ['test'],
          syntaxLanguage: 'python',
        });

        const updatedNote = await noteRepository.getById(note.id);
        expect(updatedNote?.needsSync).toBe(true);
      });
    });

    describe('content changes', () => {
      it('should update modifiedAt when content changes', async () => {
        const note = await noteService.createNote('Original content', ['test']);
        const originalModifiedAt = note.modifiedAt;

        await new Promise(resolve => setTimeout(resolve, 10));

        const updated = await noteService.updateNote(note.id, {
          content: 'Updated content',
          tags: ['test'],
        });

        expect(updated.modifiedAt).not.toBe(originalModifiedAt);
      });

      it('should NOT update modifiedAt when content is unchanged', async () => {
        const note = await noteService.createNote('Same content', ['test']);
        const originalModifiedAt = note.modifiedAt;

        await new Promise(resolve => setTimeout(resolve, 10));

        const updated = await noteService.updateNote(note.id, {
          content: 'Same content', // Same
          tags: ['test'], // Same
        });

        expect(updated.modifiedAt).toBe(originalModifiedAt);
      });
    });
  });

  describe('togglePin', () => {
    it('should update modifiedAt when pinning', async () => {
      const note = await noteService.createNote('Test content', ['test']);
      const originalModifiedAt = note.modifiedAt;

      await new Promise(resolve => setTimeout(resolve, 10));

      await noteService.togglePin(note.id);

      const updatedNote = await noteRepository.getById(note.id);
      expect(updatedNote?.pinned).toBe(true);
      expect(updatedNote?.modifiedAt).not.toBe(originalModifiedAt);
    });

    it('should set needsSync when pinning', async () => {
      const note = await noteService.createNote('Test content', ['test']);

      // Clear needsSync
      const rawNote = await noteRepository.getById(note.id);
      if (rawNote) {
        rawNote.needsSync = false;
        await noteRepository.update(rawNote, false, true);
      }

      await noteService.togglePin(note.id);

      const updatedNote = await noteRepository.getById(note.id);
      expect(updatedNote?.needsSync).toBe(true);
    });
  });
});
