import { describe, it, expect } from 'vitest';
import {
  isTimestamp,
  isUuid,
  validateAttachmentRef,
  validateConflict,
  validateInboxItem,
  validateRemoteNote,
  validateRemoteVersion,
  validateSavedSearch,
} from './syncValidation';
import type { AttachmentRef, SyncNote, SyncNoteVersion, SyncRejected, SyncSavedSearch, InboxItem } from '../types';

const ID = '3f2b8c1e-7d4a-4e9b-8a6f-1c2d3e4f5a6b';
const ATTACHMENT_ID = '9a8b7c6d-5e4f-4a3b-9c2d-1e0f9a8b7c6d';
const NOW = '2026-11-05T01:02:03.000Z';
const HOSTILE = '"><img src=x onerror=alert(1)>';
// A real digest, in the form all four clients write: standard base64 of SHA-256
const hash = 'n4bQgYhMfWWaL+qgxVrQFaO/TxsrC4Is0V1sFbDwCgg=';

function ref(overrides: Partial<AttachmentRef> = {}): AttachmentRef {
  return { id: ATTACHMENT_ID, filename: '{"ciphertext":"a"}', mimeType: 'image/png', size: 10, data: ATTACHMENT_ID, ...overrides };
}

function note(overrides: Partial<SyncNote> = {}): SyncNote {
  return {
    id: ID, createdAt: NOW, modifiedAt: NOW, content: '{"ciphertext":"a"}', tags: [], attachments: [],
    pinned: false, archived: false, deleted: false, version: 1, ...overrides,
  };
}

function reject(overrides: Record<string, unknown>) {
  return validateRemoteNote(note(overrides as Partial<SyncNote>));
}

describe('isUuid', () => {
  it('accepts lower and upper case', () => {
    expect(isUuid(ID)).toBe(true);
    expect(isUuid(ID.toUpperCase())).toBe(true);
  });

  it('rejects anything else', () => {
    for (const value of ['', 'abc', `${ID}x`, ` ${ID}`, `${ID}\n`, HOSTILE, 1, null, undefined, {}, [ID]]) {
      expect(isUuid(value)).toBe(false);
    }
  });
});

describe('isTimestamp', () => {
  it('accepts the formats clients write', () => {
    for (const value of [NOW, '2026-11-05T01:02:03Z', '2026-11-05T01:02:03+00:00', '2026-11-05T01:02:03.123456789+00:00']) {
      expect(isTimestamp(value)).toBe(true);
    }
  });

  it('rejects numbers and text', () => {
    for (const value of [1, 1730768523000, 'yesterday', '', null, undefined, {}]) {
      expect(isTimestamp(value)).toBe(false);
    }
  });
});

describe('validateAttachmentRef', () => {
  it('accepts a real reference unchanged', () => {
    expect(validateAttachmentRef(ref())).toEqual(ref());
  });

  it('rejects a hostile id or data', () => {
    expect(validateAttachmentRef(ref({ id: HOSTILE }))).toBeNull();
    expect(validateAttachmentRef(ref({ data: HOSTILE }))).toBeNull();
  });

  it('rejects a reference that is not an object', () => {
    for (const value of [null, undefined, 'x', 1, []]) {
      expect(validateAttachmentRef(value)).toBeNull();
    }
  });

  it('rejects a filename that is not a string', () => {
    expect(validateAttachmentRef(ref({ filename: 1 as unknown as string }))).toBeNull();
  });

  it('keeps real mime types, including ones with parameters', () => {
    for (const mimeType of ['image/svg+xml', 'application/vnd.ms-excel', 'text/plain', 'text/plain; charset=utf-8', 'IMAGE/PNG']) {
      expect(validateAttachmentRef(ref({ mimeType }))?.mimeType).toBe(mimeType);
    }
  });

  it('keeps the attachment but neutralises a mime type that makes no sense', () => {
    for (const mimeType of [HOSTILE, 'nonsense', '', '/', 'a/b/c', 1 as unknown as string]) {
      expect(validateAttachmentRef(ref({ mimeType }))?.mimeType).toBe('application/octet-stream');
    }
  });

  it('treats a nonsense size as unknown rather than losing the attachment', () => {
    for (const size of [-1, NaN, Infinity, '12' as unknown as number, HOSTILE as unknown as number]) {
      expect(validateAttachmentRef(ref({ size }))?.size).toBe(0);
    }
  });

  it('drops fields it does not recognise', () => {
    const value = validateAttachmentRef({ ...ref(), extra: HOSTILE }) as unknown as Record<string, unknown>;
    expect(value.extra).toBeUndefined();
  });
});

describe('validateRemoteNote', () => {
  it('accepts a normal note', () => {
    expect(validateRemoteNote(note()).ok).toBe(true);
  });

  it('accepts an upper-case id', () => {
    expect(reject({ id: ID.toUpperCase() }).ok).toBe(true);
  });

  it.each([
    ['id', { id: HOSTILE }],
    ['createdAt', { createdAt: 1 }],
    ['modifiedAt', { modifiedAt: 'soon' }],
    ['content', { content: 1 }],
    ['tags', { tags: 'x' }],
    ['a tag', { tags: [1] }],
    ['attachments', { attachments: 'x' }],
    ['version', { version: 'x' }],
    ['a negative version', { version: -1 }],
  ])('rejects a bad %s', (_name, overrides) => {
    const result = reject(overrides);
    expect(result.ok).toBe(false);
  });

  it('rejects a note when any attachment is hostile, and names the field', () => {
    const result = reject({ attachments: [ref(), ref({ data: HOSTILE })] });
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toContain('attachment');
  });

  it('rejects something that is not a note', () => {
    for (const value of [null, undefined, 'x', 1, []]) {
      expect(validateRemoteNote(value).ok).toBe(false);
    }
  });

  it('does not repeat hostile values in its reason', () => {
    const result = reject({ id: HOSTILE });
    if (!result.ok) expect(result.reason).not.toContain('<img');
  });

  it('drops a hostile display field without losing the note', () => {
    const result = reject({ syntaxLanguage: HOSTILE, color: HOSTILE });
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.value.syntaxLanguage).toBeUndefined();
      expect(result.value.color).toBeUndefined();
    }
  });

  it('keeps a language this client has not heard of', () => {
    const result = reject({ syntaxLanguage: 'zig' });
    if (result.ok) expect(result.value.syntaxLanguage).toBe('zig');
    expect(result.ok).toBe(true);
  });

  it.each(['__proto__', 'constructor', 'toString'])('drops the language %s', language => {
    const result = reject({ syntaxLanguage: language });
    if (result.ok) expect(result.value.syntaxLanguage).toBeUndefined();
    expect(result.ok).toBe(true);
  });

  it('drops bad optional timestamps', () => {
    const result = reject({ deletedAt: 1, archivedAt: 'x', lockedAt: {} });
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.value.deletedAt).toBeUndefined();
      expect(result.value.archivedAt).toBeUndefined();
      expect(result.value.lockedAt).toBeUndefined();
    }
  });

  it('reads flags strictly', () => {
    const result = reject({ pinned: 'yes', archived: 1, deleted: 'no', locked: {} });
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.value.pinned).toBe(false);
      expect(result.value.archived).toBe(false);
      expect(result.value.deleted).toBe(false);
      expect(result.value.locked).toBe(false);
    }
  });

  it('keeps hash fields of the right shape and drops the rest', () => {
    const good = reject({ contentHash: hash, parentHash: null, hashChain: [hash] });
    expect(good.ok).toBe(true);
    if (good.ok) {
      expect(good.value.contentHash).toBe(hash);
      expect(good.value.parentHash).toBeNull();
      expect(good.value.hashChain).toEqual([hash]);
    }

    const bad = reject({ contentHash: HOSTILE, parentHash: 1, hashChain: [1, hash] });
    expect(bad.ok).toBe(true);
    if (bad.ok) {
      expect(bad.value.contentHash).toBeUndefined();
      expect(bad.value.parentHash).toBeUndefined();
      expect(bad.value.hashChain).toEqual([hash]);
    }
  });
});

describe('content hashes', () => {
  it('matches what this client actually computes', async () => {
    const { cryptoService } = await import('./crypto');
    const real = await cryptoService.computeContentHash({ content: 'x' });
    const result = reject({ contentHash: real, hashChain: [real] });
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.value.contentHash).toBe(real);
      expect(result.value.hashChain).toEqual([real]);
    }
  });
});

describe('validateRemoteVersion', () => {
  function version(overrides: Record<string, unknown> = {}): SyncNoteVersion {
    return {
      versionKey: `${ID}:1`, noteId: ID, version: 1, createdAt: NOW, syncedAt: NOW,
      content: 'x', tags: [], attachments: [], reason: 'sync', ...overrides,
    } as SyncNoteVersion;
  }

  it('accepts a normal version', () => {
    expect(validateRemoteVersion(version()).ok).toBe(true);
  });

  it.each([
    ['noteId', { noteId: HOSTILE }],
    ['version', { version: 'x' }],
    ['createdAt', { createdAt: 1 }],
    ['content', { content: null }],
    ['attachments', { attachments: [ref({ id: HOSTILE })] }],
  ])('rejects a bad %s', (_name, overrides) => {
    expect(validateRemoteVersion(version(overrides)).ok).toBe(false);
  });

  it('accepts a missing syncedAt from an older server', () => {
    expect(validateRemoteVersion(version({ syncedAt: undefined })).ok).toBe(true);
  });

  it('drops a hostile language', () => {
    const result = validateRemoteVersion(version({ syntaxLanguage: HOSTILE }));
    expect(result.ok).toBe(true);
    if (result.ok) expect(result.value.syntaxLanguage).toBeUndefined();
  });
});

describe('validateConflict', () => {
  function rejected(overrides: Record<string, unknown> = {}): SyncRejected {
    return {
      id: ID, reason: 'conflict', serverModifiedAt: NOW, serverContent: 'x', serverTags: [],
      serverVersion: 2, serverAttachments: [], serverPinned: false, ...overrides,
    } as SyncRejected;
  }

  it('accepts a normal conflict', () => {
    expect(validateConflict(rejected()).ok).toBe(true);
  });

  it.each([
    ['id', { id: HOSTILE }],
    ['serverModifiedAt', { serverModifiedAt: 1 }],
    ['serverContent', { serverContent: 1 }],
    ['serverTags', { serverTags: [1] }],
    ['serverAttachments', { serverAttachments: [ref({ data: HOSTILE })] }],
    ['serverVersion', { serverVersion: 'x' }],
  ])('rejects a bad %s', (_name, overrides) => {
    expect(validateConflict(rejected(overrides)).ok).toBe(false);
  });

  it('drops a hostile language', () => {
    const result = validateConflict(rejected({ serverSyntaxLanguage: HOSTILE }));
    expect(result.ok).toBe(true);
    if (result.ok) expect(result.value.serverSyntaxLanguage).toBeUndefined();
  });

  it('accepts a conflict with no server attachments field', () => {
    const result = validateConflict(rejected({ serverAttachments: undefined }));
    expect(result.ok).toBe(true);
    if (result.ok) expect(result.value.serverAttachments).toEqual([]);
  });
});

describe('validateSavedSearch', () => {
  function search(overrides: Record<string, unknown> = {}): SyncSavedSearch {
    return { id: ID, name: 'n', query: 'q', order: 0, createdAt: NOW, modifiedAt: NOW, deleted: false, version: 1, ...overrides } as SyncSavedSearch;
  }

  it('accepts a normal search', () => {
    expect(validateSavedSearch(search()).ok).toBe(true);
  });

  it.each([
    ['id', { id: HOSTILE }],
    ['name', { name: 1 }],
    ['query', { query: null }],
    ['modifiedAt', { modifiedAt: 1 }],
    ['createdAt', { createdAt: 'x' }],
  ])('rejects a bad %s', (_name, overrides) => {
    expect(validateSavedSearch(search(overrides)).ok).toBe(false);
  });
});

describe('validateInboxItem', () => {
  function item(overrides: Record<string, unknown> = {}): InboxItem {
    return { id: ID, content: 'c', tags: [], createdAt: NOW, sizeBytes: 1, ...overrides } as InboxItem;
  }

  it('accepts a normal item', () => {
    expect(validateInboxItem(item())).toEqual(item());
  });

  it.each([
    ['id', { id: HOSTILE }],
    ['content', { content: 1 }],
    ['createdAt', { createdAt: 1 }],
    ['tags', { tags: 'x' }],
  ])('rejects a bad %s', (_name, overrides) => {
    expect(validateInboxItem(item(overrides))).toBeNull();
  });

  it('keeps only string tags and treats a bad size as zero', () => {
    const value = validateInboxItem(item({ tags: ['a', 1, ''], sizeBytes: 'x' }));
    expect(value?.tags).toEqual(['a']);
    expect(value?.sizeBytes).toBe(0);
  });
});
