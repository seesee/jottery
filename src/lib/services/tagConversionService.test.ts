import { describe, it, expect, beforeEach } from 'vitest';
import { cryptoService, encryptJSON } from './crypto';
import { decryptTags, storageToSync, syncToStorage } from './tagConversionService';

describe('tag formats', () => {
  let key: CryptoKey;

  beforeEach(async () => {
    key = await crypto.subtle.generateKey({ name: 'AES-GCM', length: 256 }, true, ['encrypt', 'decrypt']);
  });

  const syncTag = async (tag: string) => JSON.stringify(await cryptoService.encryptText(JSON.stringify(tag), key));
  const storageBlob = async (tags: string[]) => [JSON.stringify(await encryptJSON(tags, key))];

  describe('decryptTags reads every form that exists', () => {
    it('the storage format', async () => {
      expect(await decryptTags(await storageBlob(['a', 'b']), key)).toEqual(['a', 'b']);
    });

    it('the sync format', async () => {
      expect(await decryptTags([await syncTag('a'), await syncTag('b')], key)).toEqual(['a', 'b']);
    });

    it('one tag in the sync format, which looks like the storage format', async () => {
      expect(await decryptTags([await syncTag('a')], key)).toEqual(['a']);
    });

    it('nothing at all', async () => {
      expect(await decryptTags([], key)).toEqual([]);
      expect(await decryptTags(undefined, key)).toEqual([]);
    });
  });

  describe('storageToSync', () => {
    const plain = async (tags: string[]) => decryptTags(tags, key);

    it('splits the storage format', async () => {
      expect(await plain(await storageToSync(await storageBlob(['a', 'b']), key))).toEqual(['a', 'b']);
    });

    it('does not lose one tag already in the sync format', async () => {
      expect(await plain(await storageToSync([await syncTag('work')], key))).toEqual(['work']);
    });

    it('throws rather than pushing an empty list over tags it could not read', async () => {
      // Returning [] here would erase the note's tags on the server
      await expect(storageToSync(['not json'], key)).rejects.toThrow();

      const other = await crypto.subtle.generateKey({ name: 'AES-GCM', length: 256 }, true, ['encrypt', 'decrypt']);
      await expect(storageToSync(await storageBlob(['a']), other)).rejects.toThrow();
    });
  });

  describe('syncToStorage', () => {
    it('accepts the storage format, which web and Android versions on the server use', async () => {
      const stored = await syncToStorage(await storageBlob(['a', 'b']), key);

      expect(stored).toHaveLength(1);
      expect(await decryptTags(stored, key)).toEqual(['a', 'b']);
    });
  });
});
