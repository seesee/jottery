/**
 * Tests for searchService
 * Focuses on search functionality and sorting behaviour
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { searchNotes, indexNotes } from './searchService';
import type { DecryptedNote } from '../types';

// Helper to create test notes
function createTestNote(overrides: Partial<DecryptedNote> = {}): DecryptedNote {
  const now = new Date().toISOString();
  return {
    id: `note-${Math.random().toString(36).substr(2, 9)}`,
    createdAt: now,
    modifiedAt: now,
    content: 'Test content',
    tags: [],
    attachments: [],
    pinned: false,
    deleted: false,
    version: 0,
    wordWrap: true,
    syntaxLanguage: 'plain',
    decryptedAt: Date.now(),
    ...overrides,
  };
}

describe('searchService', () => {
  describe('searchNotes sorting', () => {
    let notes: DecryptedNote[];

    beforeEach(() => {
      // Create notes with different timestamps and pinned states
      notes = [
        createTestNote({
          id: 'note-1',
          content: 'First note',
          modifiedAt: '2024-01-01T10:00:00.000Z',
          createdAt: '2024-01-01T09:00:00.000Z',
          pinned: false,
        }),
        createTestNote({
          id: 'note-2',
          content: 'Second note',
          modifiedAt: '2024-01-03T10:00:00.000Z',
          createdAt: '2024-01-02T09:00:00.000Z',
          pinned: false,
        }),
        createTestNote({
          id: 'note-3',
          content: 'Third note',
          modifiedAt: '2024-01-02T10:00:00.000Z',
          createdAt: '2024-01-03T09:00:00.000Z',
          pinned: false,
        }),
      ];

      // Index the notes for search
      indexNotes(notes);
    });

    describe('sort order: recent', () => {
      it('should sort by modifiedAt descending (most recent first)', async () => {
        const results = await searchNotes('', notes, 'recent');

        expect(results).toHaveLength(3);
        expect(results[0].id).toBe('note-2'); // Most recently modified
        expect(results[1].id).toBe('note-3');
        expect(results[2].id).toBe('note-1'); // Oldest modified
      });

      it('should place pinned notes first, then sort by modifiedAt', async () => {
        notes[0].pinned = true; // Pin the oldest note

        const results = await searchNotes('', notes, 'recent');

        expect(results).toHaveLength(3);
        expect(results[0].id).toBe('note-1'); // Pinned note first
        expect(results[0].pinned).toBe(true);
        expect(results[1].id).toBe('note-2'); // Then most recent
        expect(results[2].id).toBe('note-3');
      });

      it('should sort pinned notes by modifiedAt among themselves', async () => {
        notes[0].pinned = true;
        notes[2].pinned = true;

        const results = await searchNotes('', notes, 'recent');

        // Pinned notes first (sorted by modifiedAt)
        expect(results[0].id).toBe('note-3'); // Pinned, more recent
        expect(results[1].id).toBe('note-1'); // Pinned, older
        // Then unpinned
        expect(results[2].id).toBe('note-2');
      });
    });

    describe('sort order: oldest', () => {
      it('should sort by modifiedAt ascending (oldest first)', async () => {
        const results = await searchNotes('', notes, 'oldest');

        expect(results).toHaveLength(3);
        expect(results[0].id).toBe('note-1'); // Oldest modified
        expect(results[1].id).toBe('note-3');
        expect(results[2].id).toBe('note-2'); // Most recently modified
      });

      it('should still place pinned notes first', async () => {
        notes[1].pinned = true; // Pin the most recently modified note

        const results = await searchNotes('', notes, 'oldest');

        expect(results[0].id).toBe('note-2'); // Pinned note first
        expect(results[1].id).toBe('note-1'); // Then oldest
        expect(results[2].id).toBe('note-3');
      });
    });

    describe('sort order: created', () => {
      it('should sort by createdAt descending (newest created first)', async () => {
        const results = await searchNotes('', notes, 'created');

        expect(results).toHaveLength(3);
        expect(results[0].id).toBe('note-3'); // Newest created
        expect(results[1].id).toBe('note-2');
        expect(results[2].id).toBe('note-1'); // Oldest created
      });
    });

    describe('sort order: alpha', () => {
      it('should sort alphabetically by first line of content', async () => {
        notes[0].content = 'Zebra note';
        notes[1].content = 'Apple note';
        notes[2].content = 'Mango note';

        const results = await searchNotes('', notes, 'alpha');

        expect(results).toHaveLength(3);
        expect(results[0].content).toBe('Apple note');
        expect(results[1].content).toBe('Mango note');
        expect(results[2].content).toBe('Zebra note');
      });

      it('should still place pinned notes first in alpha sort', async () => {
        notes[0].content = 'Zebra note';
        notes[0].pinned = true;
        notes[1].content = 'Apple note';
        notes[2].content = 'Mango note';

        const results = await searchNotes('', notes, 'alpha');

        expect(results[0].content).toBe('Zebra note'); // Pinned first despite being last alphabetically
        expect(results[1].content).toBe('Apple note');
        expect(results[2].content).toBe('Mango note');
      });
    });

    describe('sorting with search query', () => {
      it('should sort filtered results correctly', async () => {
        notes[0].content = 'Apple fruit note';
        notes[1].content = 'Banana fruit note';
        notes[2].content = 'Cherry dessert note';

        // Re-index with new content
        indexNotes(notes);

        const results = await searchNotes('fruit', notes, 'alpha');

        // Should only return notes containing "fruit", sorted alphabetically
        expect(results).toHaveLength(2);
        expect(results[0].content).toBe('Apple fruit note');
        expect(results[1].content).toBe('Banana fruit note');
      });
    });

    describe('sorting consistency after note updates', () => {
      it('should maintain correct sort order when notes are updated', async () => {
        // Simulate updating a note's modifiedAt (like after saving)
        const updatedNotes = [...notes];
        updatedNotes[0] = {
          ...updatedNotes[0],
          modifiedAt: '2024-01-05T10:00:00.000Z', // Now the most recent
        };

        const results = await searchNotes('', updatedNotes, 'recent');

        expect(results[0].id).toBe('note-1'); // Now most recent after update
        expect(results[1].id).toBe('note-2');
        expect(results[2].id).toBe('note-3');
      });

      it('should re-sort when pin state changes', async () => {
        // Initially sorted by recent
        let results = await searchNotes('', notes, 'recent');
        expect(results[0].id).toBe('note-2');

        // Now pin the oldest note
        const updatedNotes = [...notes];
        updatedNotes[0] = { ...updatedNotes[0], pinned: true };

        results = await searchNotes('', updatedNotes, 'recent');

        // Pinned note should now be first
        expect(results[0].id).toBe('note-1');
        expect(results[0].pinned).toBe(true);
      });
    });
  });

  describe('searchNotes filtering', () => {
    let notes: DecryptedNote[];

    beforeEach(() => {
      notes = [
        createTestNote({
          id: 'note-1',
          content: 'Meeting notes for project',
          tags: ['work', 'meetings'],
        }),
        createTestNote({
          id: 'note-2',
          content: 'Shopping list',
          tags: ['personal'],
        }),
        createTestNote({
          id: 'note-3',
          content: 'Project deadline reminder',
          tags: ['work', 'reminders'],
        }),
      ];

      indexNotes(notes);
    });

    it('should filter by tag', async () => {
      const results = await searchNotes('#work', notes, 'recent');

      expect(results).toHaveLength(2);
      expect(results.every(n => n.tags.includes('work'))).toBe(true);
    });

    it('should filter by text content', async () => {
      const results = await searchNotes('project', notes, 'recent');

      expect(results).toHaveLength(2);
      expect(results.some(n => n.id === 'note-1')).toBe(true);
      expect(results.some(n => n.id === 'note-3')).toBe(true);
    });

    it('should return all notes when query is empty', async () => {
      const results = await searchNotes('', notes, 'recent');

      expect(results).toHaveLength(3);
    });
  });

  describe('Archive modifier', () => {
    let notes: DecryptedNote[];

    beforeEach(() => {
      notes = [
        createTestNote({
          id: 'note-1',
          content: 'Active note 1',
          archived: false,
        }),
        createTestNote({
          id: 'note-2',
          content: 'Archived note 1',
          archived: true,
          archivedAt: '2024-01-02T10:00:00.000Z',
        }),
        createTestNote({
          id: 'note-3',
          content: 'Active note 2',
          archived: false,
        }),
        createTestNote({
          id: 'note-4',
          content: 'Archived note 2',
          archived: true,
          archivedAt: '2024-01-03T10:00:00.000Z',
        }),
      ];

      indexNotes(notes);
    });

    it('should exclude archived notes by default', async () => {
      const results = await searchNotes('', notes, 'recent');

      expect(results).toHaveLength(2);
      expect(results.every(n => !n.archived)).toBe(true);
      expect(results.some(n => n.id === 'note-1')).toBe(true);
      expect(results.some(n => n.id === 'note-3')).toBe(true);
    });

    it('should show only archived notes with archive:true', async () => {
      const results = await searchNotes('archive:true', notes, 'recent');

      expect(results).toHaveLength(2);
      expect(results.every(n => n.archived === true)).toBe(true);
      expect(results.some(n => n.id === 'note-2')).toBe(true);
      expect(results.some(n => n.id === 'note-4')).toBe(true);
    });

    it('should show only active notes with archive:false', async () => {
      const results = await searchNotes('archive:false', notes, 'recent');

      expect(results).toHaveLength(2);
      expect(results.every(n => !n.archived)).toBe(true);
      expect(results.some(n => n.id === 'note-1')).toBe(true);
      expect(results.some(n => n.id === 'note-3')).toBe(true);
    });

    it('should show only archived notes with archived shorthand', async () => {
      const results = await searchNotes('archived', notes, 'recent');

      expect(results).toHaveLength(2);
      expect(results.every(n => n.archived === true)).toBe(true);
      expect(results.some(n => n.id === 'note-2')).toBe(true);
      expect(results.some(n => n.id === 'note-4')).toBe(true);
    });

    it('should combine archive modifier with text search', async () => {
      const results = await searchNotes('archive:true note 2', notes, 'recent');

      expect(results).toHaveLength(1);
      expect(results[0].id).toBe('note-4');
      expect(results[0].archived).toBe(true);
    });

    it('should combine archive modifier with tag search', async () => {
      const testNotes = [
        ...notes,
        createTestNote({
          id: 'note-5',
          content: 'Tagged archived note',
          tags: ['important'],
          archived: true,
          archivedAt: '2024-01-04T10:00:00.000Z',
        }),
      ];

      indexNotes(testNotes);

      const results = await searchNotes('archive:true #important', testNotes, 'recent');

      expect(results).toHaveLength(1);
      expect(results[0].id).toBe('note-5');
      expect(results[0].archived).toBe(true);
      expect(results[0].tags).toContain('important');
    });
  });
});
