/**
 * Imported files are untrusted: they can come from another app or another person,
 * and one bad value used to blank the whole note list on every load.
 */
import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { importNotes } from './exportService';
import { noteService } from './noteService';
import { noteRepository } from './noteRepository';
import { searchService } from './searchService';
import { keyManager } from './keyManager';
import { cryptoService } from './crypto';
import { getDB, STORES } from './db';
import { initTestDB, cleanupTestDB, createTestNote } from '../../test/db-utils';
import type { ExportData } from '../types';

const GOOD_ID = 'a0000000-0000-4000-8000-000000000030';
const HOSTILE = '"><img src=x onerror=alert(1)>';
const DATE = '2025-01-02T03:04:05.000Z';

function file(notes: unknown[]): ExportData {
  return { version: '1.0', exportDate: DATE, notes } as unknown as ExportData;
}

function exported(overrides: Record<string, unknown> = {}) {
  return { id: GOOD_ID, createdAt: DATE, modifiedAt: DATE, content: 'text', tags: ['ok'], attachments: [], pinned: false, ...overrides };
}

describe('importing notes', () => {
  beforeEach(async () => {
    const { masterKey } = await initTestDB();
    keyManager.setMasterKey({ key: masterKey, keyBytes: new Uint8Array(32), derivedAt: Date.now() });
  });

  afterEach(async () => {
    keyManager.clearMasterKey();
    await cleanupTestDB();
  });

  const imported = async () => (await noteRepository.getAll())[0];

  it('keeps a well-formed note as it is', async () => {
    const result = await importNotes(file([exported({ pinned: true, syntaxLanguage: 'python', color: 'red' })]), 'skip');

    expect(result.imported).toBe(1);
    expect(result.errors).toEqual([]);
    const note = await imported();
    expect(note.id).toBe(GOOD_ID);
    expect(note.createdAt).toBe(DATE);
    expect(note.modifiedAt).toBe(DATE);
    expect(note.pinned).toBe(true);
    expect(note.syntaxLanguage).toBe('python');
    expect(note.color).toBe('red');
  });

  describe('dates', () => {
    it('stores an epoch timestamp as an ISO string', async () => {
      await importNotes(file([exported({ modifiedAt: 1735787045000, createdAt: 1735787045000 })]), 'skip');
      const note = await imported();
      expect(note.modifiedAt).toBe('2025-01-02T03:04:05.000Z');
      expect(note.createdAt).toBe('2025-01-02T03:04:05.000Z');
    });

    it('reads another valid date format', async () => {
      await importNotes(file([exported({ modifiedAt: '2025-01-02T03:04:05+00:00' })]), 'skip');
      expect((await imported()).modifiedAt).toBe(DATE);
    });

    it.each([['text', 'yesterday'], ['an object', {}], ['null', null], ['a huge number', 1e20], ['NaN', NaN]])(
      'falls back to now for %s',
      async (_name, value) => {
        const before = Date.now();
        await importNotes(file([exported({ modifiedAt: value })]), 'skip');
        const stored = Date.parse((await imported()).modifiedAt);
        expect(stored).toBeGreaterThanOrEqual(before);
      }
    );

    it('still lists every note, which one numeric date used to stop', async () => {
      const notes = Array.from({ length: 8 }, (_, i) => exported({ id: `a0000000-0000-4000-8000-00000000004${i}` }));
      await importNotes(file([...notes, exported({ modifiedAt: 1 })]), 'skip');
      expect(await noteService.getNotesForList('recent')).toHaveLength(9);
    });
  });

  describe('tags', () => {
    it('keeps only non-empty strings', async () => {
      await importNotes(file([exported({ tags: [1, '', '  ', null, {}, 'kept'] })]), 'skip');
      const [note] = await noteService.getAllNotes();
      expect(note.tags).toEqual(['kept']);
    });

    it('accepts a single string', async () => {
      await importNotes(file([exported({ tags: 'solo' })]), 'skip');
      const [note] = await noteService.getAllNotes();
      expect(note.tags).toEqual(['solo']);
    });
  });

  describe('fields an imported file does not get to decide', () => {
    it('never opens a note straight into a preview', async () => {
      await importNotes(file([exported({ showPreview: true, syntaxLanguage: 'html' })]), 'skip');
      expect((await imported()).showPreview).toBe(false);
    });

    it('drops a hostile language and colour', async () => {
      await importNotes(file([exported({ syntaxLanguage: HOSTILE, color: HOSTILE })]), 'skip');
      const note = await imported();
      expect(note.syntaxLanguage).not.toBe(HOSTILE);
      expect(note.color).toBeUndefined();
    });

    it('gives a bad id a new one', async () => {
      await importNotes(file([exported({ id: HOSTILE })]), 'skip');
      expect((await imported()).id).toMatch(/^[0-9a-f-]{36}$/);
    });

    it('gives a missing id a new one', async () => {
      await importNotes(file([exported({ id: undefined })]), 'skip');
      expect((await imported()).id).toMatch(/^[0-9a-f-]{36}$/);
    });

    it('reads flags strictly', async () => {
      await importNotes(file([exported({ pinned: 'yes', locked: 1 })]), 'skip');
      const note = await imported();
      expect(note.pinned).toBe(false);
      expect(note.locked).toBe(false);
    });
  });

  describe('attachments', () => {
    const attachment = (overrides = {}) => ({ filename: 'a.png', mimeType: 'image/png', data: 'AAAA', ...overrides });

    it('imports a good attachment', async () => {
      const result = await importNotes(file([exported({ attachments: [attachment()] })]), 'skip');
      expect(result.attachments).toBe(1);
      expect((await imported()).attachments[0].mimeType).toBe('image/png');
    });

    it('neutralises a hostile mime type', async () => {
      await importNotes(file([exported({ attachments: [attachment({ mimeType: HOSTILE })] })]), 'skip');
      expect((await imported()).attachments[0].mimeType).toBe('application/octet-stream');
    });

    it('reports data that is not base64, rather than dropping it silently', async () => {
      const result = await importNotes(file([exported({ attachments: [attachment({ data: '%%%' })] })]), 'skip');
      expect(result.imported).toBe(1);
      expect(result.errors).toHaveLength(1);
      expect((await imported()).attachments).toEqual([]);
    });

    it('reports a filename that is not a string', async () => {
      const result = await importNotes(file([exported({ attachments: [attachment({ filename: 1 })] })]), 'skip');
      expect(result.errors).toHaveLength(1);
    });

    it('copes with attachments that are not a list', async () => {
      const result = await importNotes(file([exported({ attachments: 'x' })]), 'skip');
      expect(result.imported).toBe(1);
    });
  });

  describe('notes that cannot be imported', () => {
    it.each([['null', null], ['a number', 1], ['text', 'x'], ['a list', []]])('reports %s and carries on', async (_name, value) => {
      const result = await importNotes(file([value, exported()]), 'skip');
      expect(result.imported).toBe(1);
      expect(result.errors).toHaveLength(1);
    });

    it('reports content that is not text', async () => {
      const result = await importNotes(file([exported({ content: 123 })]), 'skip');
      expect(result.imported).toBe(0);
      expect(result.errors).toHaveLength(1);
      expect(await noteRepository.getAll()).toEqual([]);
    });

    it('does not repeat hostile values in its report', async () => {
      const result = await importNotes(file([exported({ id: HOSTILE, content: 1 })]), 'skip');
      expect(result.errors.join(' ')).not.toContain('<img');
    });
  });
});

describe('notes already in a vault with bad values', () => {
  let key: CryptoKey;

  beforeEach(async () => {
    key = (await initTestDB()).masterKey;
    keyManager.setMasterKey({ key, keyBytes: new Uint8Array(32), derivedAt: Date.now() });
  });

  afterEach(async () => {
    keyManager.clearMasterKey();
    await cleanupTestDB();
  });

  async function store(overrides: Record<string, unknown>) {
    const note = createTestNote({
      content: JSON.stringify(await cryptoService.encryptText('text', key)),
      tags: [],
      ...overrides,
    });
    await getDB().put(STORES.NOTES, note);
    return note;
  }

  async function storeWithTags(tags: unknown[]) {
    const encrypted = await cryptoService.encryptText(JSON.stringify(tags), key);
    return store({ tags: [JSON.stringify(encrypted)] });
  }

  for (const order of ['recent', 'oldest', 'created', 'alpha'] as const) {
    it(`lists them sorted by ${order}`, async () => {
      await store({ modifiedAt: 1, createdAt: 1 });
      await store({ modifiedAt: undefined, createdAt: null });
      await store({});

      expect(await noteService.getNotesForList(order)).toHaveLength(3);
      expect(await noteService.getAllNotes(order)).toHaveLength(3);
      const batched = await noteService.getAllNotesBatched(order, 50);
      expect(batched).toHaveLength(3);
    });

    it(`survives a search sorted by ${order}`, async () => {
      await store({ modifiedAt: 1, createdAt: 1 });
      await store({});
      const notes = await noteService.getAllNotes(order);
      expect(await searchService.searchNotes('', notes, order)).toHaveLength(2);
    });
  }

  it('gives every note a string date, so the UI can use it', async () => {
    await store({ modifiedAt: 1, createdAt: null });
    const [note] = await noteService.getAllNotes();
    expect(typeof note.modifiedAt).toBe('string');
    expect(typeof note.createdAt).toBe('string');
  });

  it('leaves a valid date exactly as stored', async () => {
    await store({ modifiedAt: '2026-11-05T01:02:03.123456+00:00' });
    const [note] = await noteService.getAllNotes();
    expect(note.modifiedAt).toBe('2026-11-05T01:02:03.123456+00:00');
  });

  it('strips tags that are not strings', async () => {
    await storeWithTags([1, null, 'kept', '']);
    const [note] = await noteService.getAllNotes();
    expect(note.tags).toEqual(['kept']);
  });
});
