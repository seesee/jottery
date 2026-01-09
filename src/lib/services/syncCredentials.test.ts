/**
 * Tests for sync credentials encryption/decryption
 *
 * Tests cover:
 * - New encrypted format (jottery:v1:...)
 * - Legacy unencrypted format (backwards compatibility)
 * - Round-trip encryption/decryption
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { cryptoService } from './crypto';
import { arrayBufferToBase64 } from '../utils/base64';

// Constants matching the implementation
const ENCRYPTED_PREFIX = 'jottery:v1:';

interface SyncCredentials {
  endpoint: string;
  clientId: string;
  apiKey: string;
  salt?: string;
}

/**
 * Helper to create encrypted credentials blob (mirrors SettingsModal export logic)
 */
async function createEncryptedCredentialsBlob(
  credentials: Omit<SyncCredentials, 'salt'>,
  saltBase64: string,
  masterKey: CryptoKey
): Promise<string> {
  const json = JSON.stringify(credentials);
  const encrypted = await cryptoService.encryptText(json, masterKey);
  const encryptedJson = JSON.stringify(encrypted);
  return `${ENCRYPTED_PREFIX}${saltBase64}.${btoa(encryptedJson)}`;
}

/**
 * Helper to create legacy credentials blob
 */
function createLegacyCredentialsBlob(credentials: SyncCredentials): string {
  return btoa(JSON.stringify(credentials));
}

/**
 * Helper to parse encrypted credentials blob
 */
async function parseEncryptedCredentialsBlob(
  blob: string,
  masterKey: CryptoKey
): Promise<{ credentials: Omit<SyncCredentials, 'salt'>; salt: string }> {
  if (!blob.startsWith(ENCRYPTED_PREFIX)) {
    throw new Error('Not an encrypted credentials blob');
  }

  const payload = blob.substring(ENCRYPTED_PREFIX.length);
  const dotIndex = payload.indexOf('.');
  if (dotIndex === -1) {
    throw new Error('Invalid encrypted credentials format');
  }

  const salt = payload.substring(0, dotIndex);
  const encryptedPayload = payload.substring(dotIndex + 1);

  const encryptedJson = atob(encryptedPayload);
  const encryptedData = JSON.parse(encryptedJson);
  const credentialsJson = await cryptoService.decryptText(encryptedData, masterKey);
  const credentials = JSON.parse(credentialsJson);

  return { credentials, salt };
}

/**
 * Helper to parse legacy credentials blob
 */
function parseLegacyCredentialsBlob(blob: string): SyncCredentials {
  const json = atob(blob);
  return JSON.parse(json);
}

/**
 * Helper to detect blob format
 */
function isEncryptedFormat(blob: string): boolean {
  return blob.startsWith(ENCRYPTED_PREFIX);
}

describe('Sync Credentials Encryption', () => {
  let masterKey: CryptoKey;
  let salt: ArrayBuffer;
  let saltBase64: string;

  beforeEach(async () => {
    // Generate a proper salt
    salt = cryptoService.generateSalt();
    saltBase64 = arrayBufferToBase64(salt);

    // Derive a test master key
    masterKey = await cryptoService.deriveKey({
      password: 'test-password-123',
      salt: salt,
      iterations: 100000,
    });
  });

  describe('Encrypted Format (jottery:v1)', () => {
    it('should create encrypted credentials blob with correct prefix', async () => {
      const credentials = {
        endpoint: 'https://sync.example.com',
        clientId: 'test-client-id',
        apiKey: 'test-api-key-secret',
      };

      const blob = await createEncryptedCredentialsBlob(credentials, saltBase64, masterKey);

      expect(blob.startsWith(ENCRYPTED_PREFIX)).toBe(true);
    });

    it('should include salt in the prefix (unencrypted)', async () => {
      const credentials = {
        endpoint: 'https://sync.example.com',
        clientId: 'test-client-id',
        apiKey: 'test-api-key-secret',
      };

      const blob = await createEncryptedCredentialsBlob(credentials, saltBase64, masterKey);
      const payload = blob.substring(ENCRYPTED_PREFIX.length);
      const dotIndex = payload.indexOf('.');
      const extractedSalt = payload.substring(0, dotIndex);

      expect(extractedSalt).toBe(saltBase64);
    });

    it('should encrypt the credentials (apiKey not visible in blob)', async () => {
      const credentials = {
        endpoint: 'https://sync.example.com',
        clientId: 'test-client-id',
        apiKey: 'super-secret-api-key',
      };

      const blob = await createEncryptedCredentialsBlob(credentials, saltBase64, masterKey);

      // The API key should not be visible in the blob
      expect(blob.includes('super-secret-api-key')).toBe(false);
      expect(blob.includes(btoa('super-secret-api-key'))).toBe(false);
    });

    it('should decrypt credentials with correct key', async () => {
      const originalCredentials = {
        endpoint: 'https://sync.example.com',
        clientId: 'test-client-id',
        apiKey: 'test-api-key-secret',
      };

      const blob = await createEncryptedCredentialsBlob(originalCredentials, saltBase64, masterKey);
      const { credentials, salt } = await parseEncryptedCredentialsBlob(blob, masterKey);

      expect(credentials.endpoint).toBe(originalCredentials.endpoint);
      expect(credentials.clientId).toBe(originalCredentials.clientId);
      expect(credentials.apiKey).toBe(originalCredentials.apiKey);
      expect(salt).toBe(saltBase64);
    });

    it('should fail to decrypt with wrong key', async () => {
      const credentials = {
        endpoint: 'https://sync.example.com',
        clientId: 'test-client-id',
        apiKey: 'test-api-key-secret',
      };

      const blob = await createEncryptedCredentialsBlob(credentials, saltBase64, masterKey);

      // Derive a different key (using same salt, different password)
      const wrongKey = await cryptoService.deriveKey({
        password: 'wrong-password',
        salt: salt,
        iterations: 100000,
      });

      await expect(parseEncryptedCredentialsBlob(blob, wrongKey)).rejects.toThrow();
    });

    it('should produce different blobs for same credentials (random IV)', async () => {
      const credentials = {
        endpoint: 'https://sync.example.com',
        clientId: 'test-client-id',
        apiKey: 'test-api-key-secret',
      };

      const blob1 = await createEncryptedCredentialsBlob(credentials, saltBase64, masterKey);
      const blob2 = await createEncryptedCredentialsBlob(credentials, saltBase64, masterKey);

      // Blobs should be different due to random IV
      expect(blob1).not.toBe(blob2);

      // But both should decrypt to the same credentials
      const { credentials: creds1 } = await parseEncryptedCredentialsBlob(blob1, masterKey);
      const { credentials: creds2 } = await parseEncryptedCredentialsBlob(blob2, masterKey);

      expect(creds1).toEqual(creds2);
    });
  });

  describe('Legacy Format (backwards compatibility)', () => {
    it('should create legacy blob without prefix', () => {
      const credentials: SyncCredentials = {
        endpoint: 'https://sync.example.com',
        clientId: 'test-client-id',
        apiKey: 'test-api-key-secret',
        salt: saltBase64,
      };

      const blob = createLegacyCredentialsBlob(credentials);

      expect(blob.startsWith(ENCRYPTED_PREFIX)).toBe(false);
    });

    it('should parse legacy blob correctly', () => {
      const credentials: SyncCredentials = {
        endpoint: 'https://sync.example.com',
        clientId: 'test-client-id',
        apiKey: 'test-api-key-secret',
        salt: saltBase64,
      };

      const blob = createLegacyCredentialsBlob(credentials);
      const parsed = parseLegacyCredentialsBlob(blob);

      expect(parsed.endpoint).toBe(credentials.endpoint);
      expect(parsed.clientId).toBe(credentials.clientId);
      expect(parsed.apiKey).toBe(credentials.apiKey);
      expect(parsed.salt).toBe(credentials.salt);
    });

    it('should expose API key in legacy blob (security issue we are fixing)', () => {
      const credentials: SyncCredentials = {
        endpoint: 'https://sync.example.com',
        clientId: 'test-client-id',
        apiKey: 'super-secret-api-key',
        salt: saltBase64,
      };

      const blob = createLegacyCredentialsBlob(credentials);
      const decoded = atob(blob);

      // Legacy format exposes the API key (this is why we encrypt now)
      expect(decoded.includes('super-secret-api-key')).toBe(true);
    });
  });

  describe('Format Detection', () => {
    it('should detect encrypted format', async () => {
      const credentials = {
        endpoint: 'https://sync.example.com',
        clientId: 'test-client-id',
        apiKey: 'test-api-key-secret',
      };

      const blob = await createEncryptedCredentialsBlob(credentials, saltBase64, masterKey);

      expect(isEncryptedFormat(blob)).toBe(true);
    });

    it('should detect legacy format', () => {
      const credentials: SyncCredentials = {
        endpoint: 'https://sync.example.com',
        clientId: 'test-client-id',
        apiKey: 'test-api-key-secret',
        salt: saltBase64,
      };

      const blob = createLegacyCredentialsBlob(credentials);

      expect(isEncryptedFormat(blob)).toBe(false);
    });

    it('should handle import of either format', async () => {
      const credentials = {
        endpoint: 'https://sync.example.com',
        clientId: 'test-client-id',
        apiKey: 'test-api-key-secret',
      };

      // Create both formats
      const encryptedBlob = await createEncryptedCredentialsBlob(credentials, saltBase64, masterKey);
      const legacyBlob = createLegacyCredentialsBlob({ ...credentials, salt: saltBase64 });

      // Parse based on format detection
      if (isEncryptedFormat(encryptedBlob)) {
        const { credentials: parsed } = await parseEncryptedCredentialsBlob(encryptedBlob, masterKey);
        expect(parsed.apiKey).toBe(credentials.apiKey);
      }

      if (!isEncryptedFormat(legacyBlob)) {
        const parsed = parseLegacyCredentialsBlob(legacyBlob);
        expect(parsed.apiKey).toBe(credentials.apiKey);
      }
    });
  });

  describe('Round-trip Tests', () => {
    it('should round-trip credentials through encrypted format', async () => {
      const original = {
        endpoint: 'https://my-sync-server.com:8443/api',
        clientId: 'uuid-client-id-12345',
        apiKey: 'very-long-api-key-with-special-chars-!@#$%',
      };

      const blob = await createEncryptedCredentialsBlob(original, saltBase64, masterKey);
      const { credentials: roundTripped } = await parseEncryptedCredentialsBlob(blob, masterKey);

      expect(roundTripped).toEqual(original);
    });

    it('should handle special characters in credentials', async () => {
      const original = {
        endpoint: 'https://example.com/path?query=value&other=123',
        clientId: 'client-with-emoji-🔐',
        apiKey: 'key with spaces and "quotes" and \'apostrophes\'',
      };

      const blob = await createEncryptedCredentialsBlob(original, saltBase64, masterKey);
      const { credentials: roundTripped } = await parseEncryptedCredentialsBlob(blob, masterKey);

      expect(roundTripped).toEqual(original);
    });

    it('should handle unicode in credentials', async () => {
      const original = {
        endpoint: 'https://例え.jp/同期',
        clientId: 'クライアント-123',
        apiKey: '密钥-🔑-κλειδί',
      };

      const blob = await createEncryptedCredentialsBlob(original, saltBase64, masterKey);
      const { credentials: roundTripped } = await parseEncryptedCredentialsBlob(blob, masterKey);

      expect(roundTripped).toEqual(original);
    });
  });
});
