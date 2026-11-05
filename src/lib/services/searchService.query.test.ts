/**
 * The query syntax documented in CLAUDE.md. The cases live in a shared fixture so
 * each client can run the same ones: a saved search has to mean the same thing
 * everywhere, and the web client is the reference.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import * as fs from 'fs';
import * as path from 'path';
import { indexNotes, parseSearchQuery, removeNote, searchNotes, updateNote } from './searchService';
import type { DecryptedNote } from '../types';

const fixture = JSON.parse(
  fs.readFileSync(path.resolve(__dirname, '../../../docs/search-query-fixture.json'), 'utf-8')
) as {
  notes: Array<{ id: string; content: string; tags: string[] }>;
  cases: Array<{ query: string; expected: string[] }>;
};

function note(overrides: Partial<DecryptedNote> & { id: string }): DecryptedNote {
  return {
    createdAt: '2026-01-01T00:00:00.000Z',
    modifiedAt: '2026-01-01T00:00:00.000Z',
    content: '',
    tags: [],
    attachments: [],
    pinned: false,
    archived: false,
    deleted: false,
    version: 1,
    ...overrides,
  } as DecryptedNote;
}

const notes = fixture.notes.map(note);

async function ids(query: string, from: DecryptedNote[] = notes): Promise<string[]> {
  return (await searchNotes(query, from)).map(n => n.id).sort();
}

describe('search query syntax', () => {
  beforeEach(() => indexNotes(notes));

  for (const { query, expected } of fixture.cases) {
    it(`${JSON.stringify(query)} finds ${expected.length} note(s)`, async () => {
      expect(await ids(query)).toEqual([...expected].sort());
    });
  }

  it('returns everything for an empty query', async () => {
    expect(await ids('')).toHaveLength(notes.length);
    expect(await ids('   ')).toHaveLength(notes.length);
  });

  it('combines with modifiers', async () => {
    const withAttachment = note({ id: 'attached', content: 'dog', attachments: [{}] as DecryptedNote['attachments'] });
    expect(await ids('dog | cat has:attachment', [...notes, withAttachment])).toEqual(['attached']);
  });

  it('finds matches beyond the first thousand notes', async () => {
    const many = Array.from({ length: 1500 }, (_, i) => note({ id: `n${String(i).padStart(4, '0')}`, content: 'common word', tags: i === 1400 ? ['rare'] : [] }));
    indexNotes(many);

    expect(await ids('common #rare', many)).toEqual(['n1400']);
    expect(await ids('common', many)).toHaveLength(1500);
  });
});

describe('parseSearchQuery', () => {
  it('groups OR terms', () => {
    expect(parseSearchQuery('a | b c').groups).toEqual([
      [{ type: 'contains', value: 'a' }, { type: 'contains', value: 'b' }],
      [{ type: 'contains', value: 'c' }],
    ]);
  });

  it('types each kind of term', () => {
    expect(parseSearchQuery('plain "a phrase" pre* *suf *mid* #tag').groups?.flat()).toEqual([
      { type: 'contains', value: 'plain' },
      { type: 'phrase', value: 'a phrase' },
      { type: 'prefix', value: 'pre' },
      { type: 'suffix', value: 'suf' },
      { type: 'contains', value: 'mid' },
      { type: 'tag', value: 'tag' },
    ]);
  });

  it('keeps asterisks inside a phrase literal', () => {
    expect(parseSearchQuery('"a*"').groups).toEqual([[{ type: 'phrase', value: 'a*' }]]);
  });

  it('collects exclusions separately', () => {
    const parsed = parseSearchQuery('-cat -"two words" -#tag');
    expect(parsed.groups).toEqual([]);
    expect(parsed.excludeText).toEqual(['cat', 'two words']);
    expect(parsed.excludeTags).toEqual(['tag']);
  });

  it('treats any whitespace as a separator', () => {
    expect(parseSearchQuery('a\tb\nc').groups).toHaveLength(3);
  });

  it('ignores an empty phrase', () => {
    expect(parseSearchQuery('""').groups).toEqual([]);
  });

  it('never produces an empty exclusion, which would exclude everything', () => {
    expect(parseSearchQuery('-').excludeText).toEqual([]);
    expect(parseSearchQuery('-#').excludeTags).toEqual([]);
  });
});

describe('the search cache', () => {
  it('sees an edit made through updateNote', async () => {
    const before = note({ id: 'edited', content: 'before' });
    indexNotes([before]);
    const after = { ...before, content: 'after' };
    updateNote(after);

    expect(await ids('after', [after])).toEqual(['edited']);
    expect(await ids('before', [after])).toEqual([]);
  });

  it('never returns stale text, even if a change was not reported', async () => {
    const before = note({ id: 'edited', content: 'before' });
    indexNotes([before]);
    const after = { ...before, content: 'after' };

    expect(await ids('after', [after])).toEqual(['edited']);
  });

  it('works for a note it has not seen', async () => {
    indexNotes([]);
    expect(await ids('fresh', [note({ id: 'new', content: 'fresh note' })])).toEqual(['new']);
  });

  it('copes with a note being removed', async () => {
    const only = note({ id: 'gone', content: 'text' });
    indexNotes([only]);
    removeNote('gone');
    expect(await ids('text', [])).toEqual([]);
  });
});

describe('locking', () => {
  it('clears the cached note text', async () => {
    const search = await import('./searchService');
    const spy = vi.spyOn(search.searchService, 'indexNotes');
    const { lock } = await import('./initService');

    lock();

    expect(spy).toHaveBeenCalledWith([]);
  });
});
