import { describe, it, expect, beforeEach, vi, type Mock } from 'vitest';
import { get } from 'svelte/store';

vi.mock('./syncClient', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./syncClient')>();
  return { ...actual, fetchAttachment: vi.fn() };
});
vi.mock('./syncRepository', () => ({
  syncRepository: { getMetadata: vi.fn() },
}));
vi.mock('./keyManager', () => ({
  keyManager: { getMasterKey: vi.fn() },
}));
vi.mock('./crypto', () => ({
  cryptoService: { decryptText: vi.fn() },
}));
vi.mock('./attachmentRepository', () => ({
  attachmentRepository: { storeBlob: vi.fn(), listAllIds: vi.fn() },
}));
vi.mock('./noteService', () => ({
  noteService: { permanentlyDeleteNote: vi.fn() },
}));
vi.mock('./noteRepository', () => ({
  noteRepository: { getAllNonDeleted: vi.fn() },
}));

import {
  repairAttachment,
  deleteUndecryptableNote,
  undecryptableNotes,
  recordDecryptFailure,
  resetVaultHealth,
} from './vaultHealthService';
import { fetchAttachment, SyncApiError } from './syncClient';
import { syncRepository } from './syncRepository';
import { keyManager } from './keyManager';
import { cryptoService } from './crypto';
import { attachmentRepository } from './attachmentRepository';
import { noteService } from './noteService';
import type { Note } from '../types';

const goodMetadata = {
  syncEnabled: true,
  syncEndpoint: 'https://example.org',
  apiKey: JSON.stringify({ ciphertext: 'enc', iv: 'iv' }),
};

describe('repairAttachment', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (syncRepository.getMetadata as Mock).mockResolvedValue(goodMetadata);
    (keyManager.getMasterKey as Mock).mockReturnValue({ key: {} });
    (cryptoService.decryptText as Mock).mockResolvedValue('plain-api-key');
  });

  it('fetches the blob, stores it locally, and reports repaired', async () => {
    // 'aGVsbG8=' is base64 for 'hello'
    (fetchAttachment as Mock).mockResolvedValue({ id: 'att1', data: 'aGVsbG8=' });

    const result = await repairAttachment('att1');

    expect(result).toBe('repaired');
    expect(fetchAttachment).toHaveBeenCalledWith('https://example.org', 'plain-api-key', 'att1');
    const [storedId, storedBuffer] = (attachmentRepository.storeBlob as Mock).mock.calls[0];
    expect(storedId).toBe('att1');
    expect(new TextDecoder().decode(storedBuffer)).toBe('hello');
  });

  it('reports not-on-server when the server returns 404', async () => {
    (fetchAttachment as Mock).mockRejectedValue(new SyncApiError('Attachment fetch failed', 404, 'Attachment not found'));
    expect(await repairAttachment('att1')).toBe('not-on-server');
    expect(attachmentRepository.storeBlob).not.toHaveBeenCalled();
  });

  it('reports failed on other fetch errors', async () => {
    (fetchAttachment as Mock).mockRejectedValue(new Error('network down'));
    expect(await repairAttachment('att1')).toBe('failed');
    expect(attachmentRepository.storeBlob).not.toHaveBeenCalled();
  });

  it('reports failed when sync is not configured', async () => {
    (syncRepository.getMetadata as Mock).mockResolvedValue(null);
    expect(await repairAttachment('att1')).toBe('failed');
    expect(fetchAttachment).not.toHaveBeenCalled();
  });

  it('reports failed when the app is locked', async () => {
    (keyManager.getMasterKey as Mock).mockReturnValue(null);
    expect(await repairAttachment('att1')).toBe('failed');
    expect(fetchAttachment).not.toHaveBeenCalled();
  });
});

describe('deleteUndecryptableNote', () => {
  const note = {
    id: 'bad-note',
    createdAt: 'c',
    modifiedAt: 'm',
    content: 'x',
  } as unknown as Note;

  beforeEach(() => {
    vi.clearAllMocks();
    resetVaultHealth();
    recordDecryptFailure(note, new Error('nope'));
  });

  it('permanently deletes and prunes the store entry', async () => {
    (noteService.permanentlyDeleteNote as Mock).mockResolvedValue(undefined);
    await deleteUndecryptableNote('bad-note');
    expect(noteService.permanentlyDeleteNote).toHaveBeenCalledWith('bad-note');
    expect(get(undecryptableNotes)).toHaveLength(0);
  });

  it('keeps the entry when deletion fails', async () => {
    (noteService.permanentlyDeleteNote as Mock).mockRejectedValue(new Error('boom'));
    await expect(deleteUndecryptableNote('bad-note')).rejects.toThrow('boom');
    expect(get(undecryptableNotes)).toHaveLength(1);
  });
});
