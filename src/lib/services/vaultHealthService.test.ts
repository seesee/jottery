import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { get } from 'svelte/store';
import {
  undecryptableNotes,
  recordDecryptFailure,
  resetVaultHealth,
  scanMissingAttachments,
} from './vaultHealthService';
import { noteService } from './noteService';
import { noteRepository } from './noteRepository';
import { attachmentRepository } from './attachmentRepository';
import { keyManager } from './keyManager';
import { initTestDB, cleanupTestDB } from '../../test/db-utils';
import type { Note } from '../types';

const makeNote = (id: string): Note => ({
  id,
  createdAt: '2026-07-12T21:39:56.350Z',
  modifiedAt: '2026-07-12T21:39:56.350Z',
  content: '{"ciphertext":"abc","iv":"def"}',
  tags: [],
  attachments: [],
  pinned: false,
  deleted: false,
  version: 1,
} as unknown as Note);

describe('vaultHealthService failure store', () => {
  beforeEach(() => resetVaultHealth());

  it('records a decrypt failure with plaintext metadata only', () => {
    recordDecryptFailure(makeNote('n1'), new Error('Decryption failed'));
    const entries = get(undecryptableNotes);
    expect(entries).toHaveLength(1);
    expect(entries[0]).toMatchObject({
      id: 'n1',
      createdAt: '2026-07-12T21:39:56.350Z',
      modifiedAt: '2026-07-12T21:39:56.350Z',
      ciphertextLength: 31,
      error: 'Decryption failed',
    });
  });

  it('deduplicates by note id', () => {
    recordDecryptFailure(makeNote('n1'), new Error('a'));
    recordDecryptFailure(makeNote('n1'), new Error('b'));
    expect(get(undecryptableNotes)).toHaveLength(1);
  });

  it('stringifies non-Error failures', () => {
    recordDecryptFailure(makeNote('n2'), 'plain string');
    expect(get(undecryptableNotes)[0].error).toBe('plain string');
  });

  it('reset clears all entries', () => {
    recordDecryptFailure(makeNote('n1'), new Error('a'));
    resetVaultHealth();
    expect(get(undecryptableNotes)).toHaveLength(0);
  });
});

describe('scanMissingAttachments', () => {
  beforeEach(async () => {
    const testDB = await initTestDB();
    keyManager.setMasterKey({
      key: testDB.masterKey,
      keyBytes: new Uint8Array(32),
      derivedAt: Date.now(),
    });
    resetVaultHealth();
  });

  afterEach(async () => {
    keyManager.clearMasterKey();
    await cleanupTestDB();
  });

  const attachmentRef = (id: string) => ({
    id,
    filename: 'file.png',
    mimeType: 'image/png',
    size: 10,
    data: id,
  });

  it('reports refs with no stored blob, with title from the owning note', async () => {
    await noteService.createNote('Shopping list\nmilk', [], {
      attachments: [attachmentRef('att-present'), attachmentRef('att-missing')],
    });
    await attachmentRepository.storeBlob('att-present', new ArrayBuffer(4));

    const missing = await scanMissingAttachments();
    expect(missing).toHaveLength(1);
    expect(missing[0].attachmentId).toBe('att-missing');
    expect(missing[0].noteTitle).toBe('Shopping list');
  });

  it('uses null title when the owning note cannot be decrypted', async () => {
    const good = await noteService.createNote('seed', []);
    const corrupt: Note = {
      ...good,
      id: 'corrupt-with-attachment',
      content: JSON.stringify({ ciphertext: 'AAAAAAAAAAAAAAAAAAAAAAAA', iv: 'AAAAAAAAAAAAAAAA' }),
      tags: [],
      attachments: [attachmentRef('att-orphaned')],
    };
    await noteRepository.create(corrupt);

    const missing = await scanMissingAttachments();
    expect(missing).toHaveLength(1);
    expect(missing[0].attachmentId).toBe('att-orphaned');
    expect(missing[0].noteTitle).toBeNull();
  });

  it('returns empty when every referenced blob is stored', async () => {
    await noteService.createNote('note', [], { attachments: [attachmentRef('att-ok')] });
    await attachmentRepository.storeBlob('att-ok', new ArrayBuffer(4));
    expect(await scanMissingAttachments()).toEqual([]);
  });
});
