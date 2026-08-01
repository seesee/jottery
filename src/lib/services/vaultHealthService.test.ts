import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { undecryptableNotes, recordDecryptFailure, resetVaultHealth } from './vaultHealthService';
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
