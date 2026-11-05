/**
 * Tests for exportService
 * Focuses on export functionality, progress callbacks, and chunked JSON generation
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { exportAllNotes, exportNotes, downloadExport, importNotes, type ExportProgressCallback } from './exportService';
import { noteService } from './noteService';
import { noteRepository } from './noteRepository';
import type { Note } from '../types';
import { keyManager } from './keyManager';
import { cryptoService } from './crypto';
import { initTestDB, cleanupTestDB } from '../../test/db-utils';

describe('exportService', () => {
  let masterKey: CryptoKey;

  beforeEach(async () => {
    const testDB = await initTestDB();
    masterKey = testDB.masterKey;

    // Set up authenticated key manager
    keyManager.setMasterKey({
      key: masterKey,
      keyBytes: new Uint8Array(32),
      derivedAt: Date.now(),
    });
  });

  afterEach(async () => {
    keyManager.clearMasterKey();
    await cleanupTestDB();
  });

  describe('exportAllNotes, for someone moving away', () => {
    it('includes archived notes', async () => {
      const note = await noteService.createNote('in the archive');
      await noteRepository.archive(note.id);

      const data = await exportAllNotes();

      expect(data.notes.map(n => n.id)).toContain(note.id);
      expect(data.notes.find(n => n.id === note.id)?.archived).toBe(true);
    });

    it('leaves out what is in the recycle bin', async () => {
      const note = await noteService.createNote('binned');
      await noteRepository.softDelete(note.id);

      expect((await exportAllNotes()).notes.map(n => n.id)).not.toContain(note.id);
    });

    it('says when an attachment could not be read', async () => {
      const note = await noteService.createNote('with a lost file');
      const stored = await noteRepository.getById(note.id) as Note;
      await noteRepository.update({
        ...stored,
        attachments: [{ id: 'gone', filename: 'name', mimeType: 'text/plain', size: 1, data: 'gone' }],
      }, false, true);

      expect((await exportAllNotes()).skipped).toEqual({ notes: 0, attachments: 1 });
    });

    it('says nothing when nothing was left out', async () => {
      await noteService.createNote('fine');

      expect((await exportAllNotes()).skipped).toBeUndefined();
    });

    it('brings an archived note back archived', async () => {
      const note = await noteService.createNote('archived', [], { id: 'a0000000-0000-4000-8000-000000000099' });
      await noteRepository.archive(note.id);
      const data = await exportAllNotes();
      await noteRepository.delete(note.id);

      await importNotes(data);

      expect((await noteRepository.getById(note.id))?.archived).toBe(true);
    });
  });

  describe('exportAllNotes', () => {
    it('should export all notes with correct structure', async () => {
      await noteService.createNote('Note 1', ['tag1']);
      await noteService.createNote('Note 2', ['tag2']);

      const exportData = await exportAllNotes();

      expect(exportData.version).toBe('1.0');
      expect(exportData.exportDate).toBeDefined();
      expect(Array.isArray(exportData.notes)).toBe(true);
      expect(exportData.notes.length).toBe(2);
    });

    it('should call progress callback during export', async () => {
      await noteService.createNote('Note 1', ['tag1']);
      await noteService.createNote('Note 2', ['tag2']);
      await noteService.createNote('Note 3', ['tag3']);

      const progressCalls: Array<{
        phase: string;
        current?: number;
        total?: number;
      }> = [];

      const onProgress: ExportProgressCallback = (progress) => {
        progressCalls.push({ ...progress });
      };

      await exportAllNotes(onProgress);

      // Should have counting phase
      expect(progressCalls.some(p => p.phase === 'counting')).toBe(true);

      // Should have exporting phases
      expect(progressCalls.some(p => p.phase === 'exporting')).toBe(true);

      // Should have complete phase
      expect(progressCalls.some(p => p.phase === 'complete')).toBe(true);

      // Check progress increases
      const exportingCalls = progressCalls.filter(p => p.phase === 'exporting');
      expect(exportingCalls.length).toBe(3); // One per note

      // Verify currents are 1, 2, 3
      const currents = exportingCalls.map(p => p.current);
      expect(currents).toEqual([1, 2, 3]);

      // Total should be consistent
      const totals = exportingCalls.map(p => p.total);
      expect(totals.every(t => t === 3)).toBe(true);
    });

    it('should decrypt note content in export', async () => {
      const originalContent = 'This is my secret note content';
      await noteService.createNote(originalContent, ['secret']);

      const exportData = await exportAllNotes();

      expect(exportData.notes[0].content).toBe(originalContent);
    });

    it('should decrypt tags in export', async () => {
      const tags = ['tag1', 'tag2', 'tag3'];
      await noteService.createNote('Content', tags);

      const exportData = await exportAllNotes();

      expect(exportData.notes[0].tags).toEqual(tags);
    });

    it('should throw error when app is locked', async () => {
      keyManager.clearMasterKey();

      await expect(exportAllNotes()).rejects.toThrow('Application is locked');
    });

    it('should not include deleted notes', async () => {
      await noteService.createNote('Active note', ['active']);
      const noteToDelete = await noteService.createNote('Deleted note', ['deleted']);

      // Delete the second note
      await noteService.deleteNote(noteToDelete.id);

      const exportData = await exportAllNotes();

      expect(exportData.notes.length).toBe(1);
      expect(exportData.notes[0].content).toBe('Active note');
    });
  });

  describe('exportNotes', () => {
    it('should export only specified notes', async () => {
      const note1 = await noteService.createNote('Note 1', ['tag1']);
      const note2 = await noteService.createNote('Note 2', ['tag2']);
      await noteService.createNote('Note 3', ['tag3']);

      const exportData = await exportNotes([note1.id, note2.id]);

      expect(exportData.notes.length).toBe(2);
      expect(exportData.notes.map(n => n.content).sort()).toEqual(['Note 1', 'Note 2']);
    });
  });

  describe('a note that cannot be decrypted', () => {
    async function corrupted(field: 'content' | 'tags') {
      const note = await noteService.createNote('Secret', ['tag']);
      const stored = await noteRepository.getById(note.id);
      const otherKey = await crypto.subtle.generateKey({ name: 'AES-GCM', length: 256 }, true, ['encrypt', 'decrypt']);
      const foreign = JSON.stringify(await cryptoService.encryptText('other', otherKey));
      await noteRepository.update({ ...stored!, ...(field === 'content' ? { content: foreign } : { tags: [foreign] }) }, false, true);
      return note;
    }

    it.each(['content', 'tags'] as const)('is left out, and counted, when its %s fails', async field => {
      await corrupted(field);
      await noteService.createNote('Readable', ['fine']);

      const data = await exportAllNotes();

      expect(data.notes.map(note => note.content)).toEqual(['Readable']);
      expect(data.skipped).toEqual({ notes: 1, attachments: 0 });
      expect(JSON.stringify(data.notes)).not.toContain('ciphertext');
    });

    it('does not stop an export of chosen notes', async () => {
      const bad = await corrupted('content');
      const good = await noteService.createNote('Readable');

      const data = await exportNotes([bad.id, good.id]);

      expect(data.notes.map(note => note.content)).toEqual(['Readable']);
      expect(data.skipped?.notes).toBe(1);
    });

    it('still exports content from before notes were encrypted, which isn\'t ciphertext', async () => {
      const note = await noteService.createNote('placeholder');
      const stored = await noteRepository.getById(note.id);
      await noteRepository.update({ ...stored!, content: 'plain old text' }, false, true);

      const data = await exportAllNotes();

      expect(data.notes[0].content).toBe('plain old text');
    });

    it('still exports old plain text that happens to be valid JSON', async () => {
      const note = await noteService.createNote('placeholder');
      const stored = await noteRepository.getById(note.id);
      await noteRepository.update({ ...stored!, content: '{"a": 1}' }, false, true);

      const data = await exportAllNotes();

      expect(data.notes[0].content).toBe('{"a": 1}');
    });
  });

  describe('downloadExport', () => {
    it('should create downloadable file from export data', async () => {
      await noteService.createNote('Test note', ['test']);
      const exportData = await exportAllNotes();

      // Mock document methods
      const mockLink = {
        href: '',
        download: '',
        click: vi.fn(),
      };
      const createElementSpy = vi.spyOn(document, 'createElement').mockReturnValue(mockLink as unknown as HTMLAnchorElement);
      const appendChildSpy = vi.spyOn(document.body, 'appendChild').mockImplementation(() => mockLink as unknown as Node);
      const removeChildSpy = vi.spyOn(document.body, 'removeChild').mockImplementation(() => mockLink as unknown as Node);
      const createObjectURLSpy = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:test');
      const revokeObjectURLSpy = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});

      await downloadExport(exportData);

      expect(createElementSpy).toHaveBeenCalledWith('a');
      expect(mockLink.click).toHaveBeenCalled();
      expect(mockLink.download).toMatch(/^jottery-export-\d{4}-\d{2}-\d{2}\.json$/);
      expect(createObjectURLSpy).toHaveBeenCalled();
      expect(revokeObjectURLSpy).toHaveBeenCalled();

      // Cleanup
      createElementSpy.mockRestore();
      appendChildSpy.mockRestore();
      removeChildSpy.mockRestore();
      createObjectURLSpy.mockRestore();
      revokeObjectURLSpy.mockRestore();
    });

    it('should use custom filename when provided', async () => {
      await noteService.createNote('Test note', ['test']);
      const exportData = await exportAllNotes();

      const mockLink = {
        href: '',
        download: '',
        click: vi.fn(),
      };
      vi.spyOn(document, 'createElement').mockReturnValue(mockLink as unknown as HTMLAnchorElement);
      vi.spyOn(document.body, 'appendChild').mockImplementation(() => mockLink as unknown as Node);
      vi.spyOn(document.body, 'removeChild').mockImplementation(() => mockLink as unknown as Node);
      vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:test');
      vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});

      await downloadExport(exportData, 'my-custom-export.json');

      expect(mockLink.download).toBe('my-custom-export.json');

      vi.restoreAllMocks();
    });

    it('should handle large exports with chunked JSON', async () => {
      // Create multiple notes
      for (let i = 0; i < 10; i++) {
        await noteService.createNote(`Note ${i} with some longer content to make it bigger`, [`tag${i}`]);
      }

      const exportData = await exportAllNotes();

      let blobContent: Blob | undefined;
      vi.spyOn(document, 'createElement').mockReturnValue({
        href: '',
        download: '',
        click: vi.fn(),
      } as unknown as HTMLAnchorElement);
      vi.spyOn(document.body, 'appendChild').mockImplementation(() => ({} as Node));
      vi.spyOn(document.body, 'removeChild').mockImplementation(() => ({} as Node));
      vi.spyOn(URL, 'createObjectURL').mockImplementation((obj: Blob | MediaSource) => {
        blobContent = obj as Blob;
        return 'blob:test';
      });
      vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});

      await expect(downloadExport(exportData)).resolves.not.toThrow();
      expect(blobContent).toBeDefined();
      expect(blobContent!.type).toBe('application/json');

      vi.restoreAllMocks();
    });
  });

  describe('export data integrity', () => {
    it('should produce valid JSON with proper formatting', async () => {
      await noteService.createNote('Test note', ['test']);
      const exportData = await exportAllNotes();

      // Simulate chunked JSON generation
      const chunks: string[] = [];
      chunks.push('{\n  "version": ');
      chunks.push(JSON.stringify(exportData.version));
      chunks.push(',\n  "exportDate": ');
      chunks.push(JSON.stringify(exportData.exportDate));
      chunks.push(',\n  "notes": [\n');

      for (let i = 0; i < exportData.notes.length; i++) {
        if (i > 0) chunks.push(',\n');
        const noteJson = JSON.stringify(exportData.notes[i], null, 2);
        chunks.push('    ' + noteJson.split('\n').join('\n    '));
      }

      chunks.push('\n  ]');

      if (exportData.settings) {
        chunks.push(',\n  "settings": ');
        chunks.push(JSON.stringify(exportData.settings, null, 2).split('\n').join('\n  '));
      }

      chunks.push('\n}');

      const reconstructed = chunks.join('');

      // Should be valid JSON
      const parsed = JSON.parse(reconstructed);
      expect(parsed.version).toBe(exportData.version);
      expect(parsed.notes.length).toBe(exportData.notes.length);
    });

    it('should preserve note metadata in export', async () => {
      const note = await noteService.createNote('Test content', ['tag1', 'tag2']);

      // Update with various settings
      await noteService.updateNote(note.id, {
        content: 'Test content',
        tags: ['tag1', 'tag2'],
        pinned: true,
        wordWrap: false,
        syntaxLanguage: 'javascript',
      });

      const exportData = await exportAllNotes();
      const exportedNote = exportData.notes[0];

      expect(exportedNote.pinned).toBe(true);
      expect(exportedNote.wordWrap).toBe(false);
      expect(exportedNote.syntaxLanguage).toBe('javascript');
      expect(exportedNote.createdAt).toBeDefined();
      expect(exportedNote.modifiedAt).toBeDefined();
    });
  });
});
