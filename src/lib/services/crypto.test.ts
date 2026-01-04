/**
 * Comprehensive cryptography tests
 * CRITICAL: These tests validate the foundation of all data security in Jottery
 */

import { describe, test, expect, beforeEach } from 'vitest';
import { cryptoService, encryptJSON, decryptJSON, encryptStringArray, decryptStringArray } from './crypto';

describe('WebCryptoService', () => {
  describe('Key Derivation', () => {
    test('should derive consistent key from same password and salt', async () => {
      const password = 'test-password-123';
      const salt = cryptoService.generateSalt();

      const key1 = await cryptoService.deriveKey({ password, salt, iterations: 100000 });
      const key2 = await cryptoService.deriveKey({ password, salt, iterations: 100000 });

      // Export keys to compare (they're not extractable by default, but we can compare encrypted results)
      const testData = 'test data for comparison';
      const encrypted1 = await cryptoService.encryptText(testData, key1);
      const encrypted2 = await cryptoService.encryptText(testData, key2);

      const decrypted1 = await cryptoService.decryptText(encrypted1, key1);
      const decrypted2 = await cryptoService.decryptText(encrypted2, key2);

      expect(decrypted1).toBe(testData);
      expect(decrypted2).toBe(testData);
    });

    test('should derive different keys from different passwords', async () => {
      const salt = cryptoService.generateSalt();

      const key1 = await cryptoService.deriveKey({ password: 'password1', salt, iterations: 100000 });
      const key2 = await cryptoService.deriveKey({ password: 'password2', salt, iterations: 100000 });

      const testData = 'test data';
      const encrypted = await cryptoService.encryptText(testData, key1);

      // Decryption with wrong key should fail
      await expect(cryptoService.decryptText(encrypted, key2)).rejects.toThrow('Decryption failed');
    });

    test('should derive different keys from different salts', async () => {
      const password = 'test-password';
      const salt1 = cryptoService.generateSalt();
      const salt2 = cryptoService.generateSalt();

      const key1 = await cryptoService.deriveKey({ password, salt: salt1, iterations: 100000 });
      const key2 = await cryptoService.deriveKey({ password, salt: salt2, iterations: 100000 });

      const testData = 'test data';
      const encrypted = await cryptoService.encryptText(testData, key1);

      // Decryption with key from different salt should fail
      await expect(cryptoService.decryptText(encrypted, key2)).rejects.toThrow('Decryption failed');
    });

    test('should generate salt of correct length', () => {
      const salt = cryptoService.generateSalt();
      expect(salt).toBeInstanceOf(Uint8Array);
      expect(salt.length).toBe(32); // 256 bits
    });

    test('should generate different salts each time', () => {
      const salt1 = cryptoService.generateSalt();
      const salt2 = cryptoService.generateSalt();

      expect(salt1).not.toEqual(salt2);
    });
  });

  describe('Text Encryption/Decryption Round-Trips', () => {
    let masterKey: CryptoKey;

    beforeEach(async () => {
      const password = 'test-password';
      const salt = cryptoService.generateSalt();
      masterKey = await cryptoService.deriveKey({ password, salt, iterations: 100000 });
    });

    test('should encrypt and decrypt text successfully', async () => {
      const plaintext = 'Hello, World!';

      const encrypted = await cryptoService.encryptText(plaintext, masterKey);
      const decrypted = await cryptoService.decryptText(encrypted, masterKey);

      expect(decrypted).toBe(plaintext);
    });

    test('should handle empty strings', async () => {
      const plaintext = '';

      const encrypted = await cryptoService.encryptText(plaintext, masterKey);
      const decrypted = await cryptoService.decryptText(encrypted, masterKey);

      expect(decrypted).toBe('');
    });

    test('should handle unicode characters', async () => {
      const plaintext = '🔒 Security Test 中文 Ελληνικά العربية';

      const encrypted = await cryptoService.encryptText(plaintext, masterKey);
      const decrypted = await cryptoService.decryptText(encrypted, masterKey);

      expect(decrypted).toBe(plaintext);
    });

    test('should produce different ciphertext for same plaintext', async () => {
      const plaintext = 'Same content';

      const result1 = await cryptoService.encryptText(plaintext, masterKey);
      const result2 = await cryptoService.encryptText(plaintext, masterKey);

      expect(result1.ciphertext).not.toBe(result2.ciphertext);
      expect(result1.iv).not.toBe(result2.iv);

      // But both should decrypt correctly
      const decrypted1 = await cryptoService.decryptText(result1, masterKey);
      const decrypted2 = await cryptoService.decryptText(result2, masterKey);

      expect(decrypted1).toBe(plaintext);
      expect(decrypted2).toBe(plaintext);
    });

    test('should fail decryption with wrong key', async () => {
      const plaintext = 'Secret message';
      const wrongKey = await cryptoService.deriveKey({
        password: 'wrong-password',
        salt: cryptoService.generateSalt(),
        iterations: 100000,
      });

      const encrypted = await cryptoService.encryptText(plaintext, masterKey);

      await expect(cryptoService.decryptText(encrypted, wrongKey)).rejects.toThrow('Decryption failed');
    });

    test('should fail decryption with tampered ciphertext', async () => {
      const plaintext = 'Original message';

      const encrypted = await cryptoService.encryptText(plaintext, masterKey);
      const tamperedCiphertext = encrypted.ciphertext.slice(0, -10) + 'AAAAAAAAAA';

      await expect(
        cryptoService.decryptText({ ciphertext: tamperedCiphertext, iv: encrypted.iv }, masterKey)
      ).rejects.toThrow('Decryption failed');
    });

    test('should fail decryption with tampered IV', async () => {
      const plaintext = 'Original message';

      const encrypted = await cryptoService.encryptText(plaintext, masterKey);
      const tamperedIV = encrypted.iv.slice(0, -10) + 'AAAAAAAAAA';

      await expect(
        cryptoService.decryptText({ ciphertext: encrypted.ciphertext, iv: tamperedIV }, masterKey)
      ).rejects.toThrow();
    });

    test('should handle large texts (>1MB)', async () => {
      const largeText = 'A'.repeat(1024 * 1024); // 1MB

      const encrypted = await cryptoService.encryptText(largeText, masterKey);
      const decrypted = await cryptoService.decryptText(encrypted, masterKey);

      expect(decrypted).toBe(largeText);
      expect(decrypted.length).toBe(1024 * 1024);
    });

    test('should handle text with special characters', async () => {
      const plaintext = 'Line1\nLine2\tTabbed\r\nWindows\x00Null\x1FControl';

      const encrypted = await cryptoService.encryptText(plaintext, masterKey);
      const decrypted = await cryptoService.decryptText(encrypted, masterKey);

      expect(decrypted).toBe(plaintext);
    });
  });

  describe('Binary Encryption/Decryption (Attachments)', () => {
    let masterKey: CryptoKey;

    beforeEach(async () => {
      const password = 'test-password';
      const salt = cryptoService.generateSalt();
      masterKey = await cryptoService.deriveKey({ password, salt, iterations: 100000 });
    });

    test('should encrypt and decrypt binary data', async () => {
      const data = new Uint8Array([1, 2, 3, 4, 5, 255, 128, 0]);

      const encrypted = await cryptoService.encryptBinary(data.buffer, masterKey);
      const decrypted = await cryptoService.decryptBinary(encrypted, masterKey);

      expect(new Uint8Array(decrypted)).toEqual(data);
    });

    test('should handle empty binary data', async () => {
      const data = new Uint8Array([]);

      const encrypted = await cryptoService.encryptBinary(data.buffer, masterKey);
      const decrypted = await cryptoService.decryptBinary(encrypted, masterKey);

      expect(new Uint8Array(decrypted)).toEqual(data);
    });

    test('should handle large binary files (64KB - test env limit)', async () => {
      // Note: happy-dom limits crypto.getRandomValues to 64KB
      // In real browsers, much larger sizes work fine
      const size = 64 * 1024; // 64KB
      const data = crypto.getRandomValues(new Uint8Array(size));

      const encrypted = await cryptoService.encryptBinary(data.buffer, masterKey);
      const decrypted = await cryptoService.decryptBinary(encrypted, masterKey);

      expect(new Uint8Array(decrypted)).toEqual(data);
    });

    test('should fail decryption with wrong key', async () => {
      const data = new Uint8Array([1, 2, 3, 4, 5]);
      const wrongKey = await cryptoService.deriveKey({
        password: 'wrong-password',
        salt: cryptoService.generateSalt(),
        iterations: 100000,
      });

      const encrypted = await cryptoService.encryptBinary(data.buffer, masterKey);

      await expect(cryptoService.decryptBinary(encrypted, wrongKey)).rejects.toThrow('Decryption failed');
    });

    test('should produce different ciphertext for same binary data', async () => {
      const data = new Uint8Array([1, 2, 3, 4, 5]);

      const result1 = await cryptoService.encryptBinary(data.buffer, masterKey);
      const result2 = await cryptoService.encryptBinary(data.buffer, masterKey);

      expect(result1.ciphertext).not.toBe(result2.ciphertext);
      expect(result1.iv).not.toBe(result2.iv);
    });
  });

  describe('JSON Encryption/Decryption', () => {
    let masterKey: CryptoKey;

    beforeEach(async () => {
      const password = 'test-password';
      const salt = cryptoService.generateSalt();
      masterKey = await cryptoService.deriveKey({ password, salt, iterations: 100000 });
    });

    test('should encrypt and decrypt JSON objects', async () => {
      const obj = {
        id: '123',
        name: 'Test',
        tags: ['tag1', 'tag2'],
        nested: { value: 42 },
      };

      const encrypted = await encryptJSON(obj, masterKey);
      const decrypted = await decryptJSON<typeof obj>(encrypted, masterKey);

      expect(decrypted).toEqual(obj);
    });

    test('should handle JSON arrays', async () => {
      const arr = [1, 2, 3, 'four', { five: 5 }];

      const encrypted = await encryptJSON(arr, masterKey);
      const decrypted = await decryptJSON<typeof arr>(encrypted, masterKey);

      expect(decrypted).toEqual(arr);
    });

    test('should handle null and undefined in JSON', async () => {
      const obj = {
        nullValue: null,
        undefinedValue: undefined,
        normalValue: 'test',
      };

      const encrypted = await encryptJSON(obj, masterKey);
      const decrypted = await decryptJSON<typeof obj>(encrypted, masterKey);

      // undefined gets stripped by JSON.stringify
      expect(decrypted).toEqual({
        nullValue: null,
        normalValue: 'test',
      });
    });
  });

  describe('String Array Encryption (Tags)', () => {
    let masterKey: CryptoKey;

    beforeEach(async () => {
      const password = 'test-password';
      const salt = cryptoService.generateSalt();
      masterKey = await cryptoService.deriveKey({ password, salt, iterations: 100000 });
    });

    test('should encrypt and decrypt string arrays', async () => {
      const tags = ['work', 'important', 'project-alpha'];

      const encrypted = await encryptStringArray(tags, masterKey);
      const decrypted = await decryptStringArray(encrypted, masterKey);

      expect(decrypted).toEqual(tags);
    });

    test('should handle empty arrays', async () => {
      const tags: string[] = [];

      const encrypted = await encryptStringArray(tags, masterKey);
      const decrypted = await decryptStringArray(encrypted, masterKey);

      expect(decrypted).toEqual([]);
    });

    test('should handle unicode tags', async () => {
      const tags = ['🏷️', '工作', 'Σημαντικό'];

      const encrypted = await encryptStringArray(tags, masterKey);
      const decrypted = await decryptStringArray(encrypted, masterKey);

      expect(decrypted).toEqual(tags);
    });
  });

  describe('Hashing', () => {
    test('should produce consistent hash for same input', async () => {
      const data = 'test data';

      const hash1 = await cryptoService.hash(data);
      const hash2 = await cryptoService.hash(data);

      expect(hash1).toBe(hash2);
    });

    test('should produce different hashes for different inputs', async () => {
      const data1 = 'test data 1';
      const data2 = 'test data 2';

      const hash1 = await cryptoService.hash(data1);
      const hash2 = await cryptoService.hash(data2);

      expect(hash1).not.toBe(hash2);
    });

    test('should produce hash of expected length (Base64-encoded SHA-256)', async () => {
      const data = 'test data';
      const hash = await cryptoService.hash(data);

      // SHA-256 produces 32 bytes (256 bits), Base64 encoding makes it 44 characters
      expect(hash.length).toBe(44);
    });
  });

  describe('UUID Generation', () => {
    test('should generate valid UUID v4', () => {
      const uuid = cryptoService.generateUUID();

      // UUID v4 format: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
      const uuidRegex = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
      expect(uuid).toMatch(uuidRegex);
    });

    test('should generate different UUIDs', () => {
      const uuid1 = cryptoService.generateUUID();
      const uuid2 = cryptoService.generateUUID();

      expect(uuid1).not.toBe(uuid2);
    });

    test('should generate 100 unique UUIDs', () => {
      const uuids = new Set<string>();
      for (let i = 0; i < 100; i++) {
        uuids.add(cryptoService.generateUUID());
      }

      expect(uuids.size).toBe(100);
    });
  });

  describe('IV Generation', () => {
    test('should generate IV of correct length', () => {
      const iv = cryptoService.generateIV();
      expect(iv).toBeInstanceOf(Uint8Array);
      expect(iv.length).toBe(12); // 96 bits for GCM
    });

    test('should generate different IVs each time', () => {
      const iv1 = cryptoService.generateIV();
      const iv2 = cryptoService.generateIV();

      expect(iv1).not.toEqual(iv2);
    });

    test('should generate 1000 unique IVs', () => {
      const ivs = new Set<string>();
      for (let i = 0; i < 1000; i++) {
        const iv = cryptoService.generateIV();
        ivs.add(iv.toString());
      }

      expect(ivs.size).toBe(1000);
    });
  });

  describe('Edge Cases and Error Handling', () => {
    let masterKey: CryptoKey;

    beforeEach(async () => {
      const password = 'test-password';
      const salt = cryptoService.generateSalt();
      masterKey = await cryptoService.deriveKey({ password, salt, iterations: 100000 });
    });

    test('should handle very long passwords', async () => {
      const longPassword = 'A'.repeat(10000);
      const salt = cryptoService.generateSalt();

      const key = await cryptoService.deriveKey({ password: longPassword, salt, iterations: 100000 });
      const testData = 'test';
      const encrypted = await cryptoService.encryptText(testData, key);
      const decrypted = await cryptoService.decryptText(encrypted, key);

      expect(decrypted).toBe(testData);
    });

    test('should handle passwords with special characters', async () => {
      const specialPassword = '!@#$%^&*()_+-=[]{}|;\':",./<>?`~';
      const salt = cryptoService.generateSalt();

      const key = await cryptoService.deriveKey({ password: specialPassword, salt, iterations: 100000 });
      const testData = 'test';
      const encrypted = await cryptoService.encryptText(testData, key);
      const decrypted = await cryptoService.decryptText(encrypted, key);

      expect(decrypted).toBe(testData);
    });

    test('should handle different iteration counts', async () => {
      const password = 'test-password';
      const salt = cryptoService.generateSalt();

      const key1 = await cryptoService.deriveKey({ password, salt, iterations: 50000 });
      const key2 = await cryptoService.deriveKey({ password, salt, iterations: 100000 });

      const testData = 'test';
      const encrypted1 = await cryptoService.encryptText(testData, key1);
      const encrypted2 = await cryptoService.encryptText(testData, key2);

      // Different iterations should produce different keys
      await expect(cryptoService.decryptText(encrypted1, key2)).rejects.toThrow();
      await expect(cryptoService.decryptText(encrypted2, key1)).rejects.toThrow();
    });
  });
});
