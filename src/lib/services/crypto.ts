/**
 * Cryptography service implementation using Web Crypto API
 * Implements AES-256-GCM encryption with PBKDF2 key derivation
 */

import type {
  CryptoService,
  KeyDerivationParams,
  EncryptionResult,
  DecryptionParams,
} from '../types';
import type { Note, Attachment } from '../types/models';
import { arrayBufferToBase64, base64ToArrayBuffer } from '../utils/base64';
import { CRYPTO_PBKDF2_ITERATIONS, CRYPTO_WRAPPING_ITERATIONS } from '../constants';

/**
 * Maximum number of ancestor hashes to keep in the hash chain.
 * Older history can be retrieved from note_versions if needed.
 */
export const MAX_HASH_CHAIN_LENGTH = 50;

const ALGORITHM = 'AES-GCM';
const KEY_LENGTH = 256;
const IV_LENGTH = 12; // 96 bits recommended for GCM
const SALT_LENGTH = 32; // 256 bits

/**
 * Convert string to ArrayBuffer
 */
function stringToArrayBuffer(str: string): ArrayBuffer {
  return new TextEncoder().encode(str).buffer;
}

/**
 * Convert ArrayBuffer to string
 */
function arrayBufferToString(buffer: ArrayBuffer): string {
  return new TextDecoder().decode(buffer);
}

class WebCryptoService implements CryptoService {
  /**
   * Derive a cryptographic key from password using PBKDF2
   */
  async deriveKey(params: KeyDerivationParams): Promise<CryptoKey> {
    const { password, salt, iterations, extractable = false } = params;

    // Import password as key material
    const passwordKey = await crypto.subtle.importKey(
      'raw',
      stringToArrayBuffer(password),
      { name: 'PBKDF2' },
      false,
      ['deriveBits', 'deriveKey']
    );

    // Derive AES-GCM key
    const key = await crypto.subtle.deriveKey(
      {
        name: 'PBKDF2',
        salt: salt as BufferSource,
        iterations: iterations || CRYPTO_PBKDF2_ITERATIONS,
        hash: 'SHA-256',
      },
      passwordKey,
      {
        name: ALGORITHM,
        length: KEY_LENGTH,
      },
      extractable, // Extractable if needed for backup (JWE operations)
      ['encrypt', 'decrypt']
    );

    return key;
  }

  /**
   * Encrypt text data
   */
  async encryptText(plaintext: string, key: CryptoKey): Promise<EncryptionResult> {
    const iv = this.generateIV();
    const data = stringToArrayBuffer(plaintext);

    const ciphertext = await crypto.subtle.encrypt(
      {
        name: ALGORITHM,
        iv: iv as BufferSource,
      },
      key,
      data
    );

    return {
      ciphertext: arrayBufferToBase64(ciphertext),
      iv: arrayBufferToBase64(iv),
    };
  }

  /**
   * Decrypt text data
   */
  async decryptText(params: DecryptionParams, key: CryptoKey): Promise<string> {
    const { ciphertext, iv } = params;

    try {
      const decrypted = await crypto.subtle.decrypt(
        {
          name: ALGORITHM,
          iv: base64ToArrayBuffer(iv),
        },
        key,
        base64ToArrayBuffer(ciphertext)
      );

      return arrayBufferToString(decrypted);
    } catch (error) {
      throw new Error('Decryption failed. Invalid key or corrupted data.');
    }
  }

  /**
   * Encrypt binary data (for attachments)
   */
  async encryptBinary(data: ArrayBuffer, key: CryptoKey): Promise<EncryptionResult> {
    const iv = this.generateIV();

    const ciphertext = await crypto.subtle.encrypt(
      {
        name: ALGORITHM,
        iv: iv as BufferSource,
      },
      key,
      data
    );

    return {
      ciphertext: arrayBufferToBase64(ciphertext),
      iv: arrayBufferToBase64(iv),
    };
  }

  /**
   * Decrypt binary data (for attachments)
   */
  async decryptBinary(params: DecryptionParams, key: CryptoKey): Promise<ArrayBuffer> {
    const { ciphertext, iv } = params;

    try {
      const decrypted = await crypto.subtle.decrypt(
        {
          name: ALGORITHM,
          iv: base64ToArrayBuffer(iv),
        },
        key,
        base64ToArrayBuffer(ciphertext)
      );

      return decrypted;
    } catch (error) {
      throw new Error('Decryption failed. Invalid key or corrupted data.');
    }
  }

  /**
   * Generate random salt for key derivation
   */
  generateSalt(): Uint8Array {
    return crypto.getRandomValues(new Uint8Array(SALT_LENGTH));
  }

  /**
   * Generate initialization vector for encryption
   */
  generateIV(): Uint8Array {
    return crypto.getRandomValues(new Uint8Array(IV_LENGTH));
  }

  /**
   * Hash data using SHA-256 (for sync conflict detection)
   */
  async hash(data: string): Promise<string> {
    const buffer = stringToArrayBuffer(data);
    const hashBuffer = await crypto.subtle.digest('SHA-256', buffer);
    return arrayBufferToBase64(hashBuffer);
  }

  /**
   * Compute content hash for a note.
   * This hash uniquely identifies the note's content state for conflict detection.
   * Includes all fields that constitute a "change" to the note.
   *
   * @param note - The note to hash (with encrypted content/tags)
   * @returns SHA-256 hash of the note's change-triggering fields
   */
  async computeContentHash(note: Partial<Note>): Promise<string> {
    // Build a deterministic representation of change-triggering fields
    const hashData = {
      // Core encrypted content (already encrypted strings)
      content: note.content ?? '',
      tags: note.tags ?? [],
      // Attachment references (sorted IDs for consistency)
      attachments: (note.attachments ?? []).map((a: Attachment) => a.id).sort(),
      // UI preferences that constitute a change
      pinned: note.pinned ?? false,
      archived: note.archived ?? false,
      locked: note.locked ?? false,
      syntaxLanguage: note.syntaxLanguage ?? null,
      wordWrap: note.wordWrap ?? true,
      showPreview: note.showPreview ?? false,
      color: note.color ?? null,
    };

    // JSON.stringify produces deterministic output for this simple object
    const data = JSON.stringify(hashData);
    return this.hash(data);
  }

  // ===========================================================================
  // Envelope Encryption Methods
  // ===========================================================================

  /**
   * Generate a random 256-bit master key for envelope encryption
   */
  generateMasterKey(): Uint8Array {
    return crypto.getRandomValues(new Uint8Array(KEY_LENGTH / 8));
  }

  /**
   * Derive a wrapping key from password + userId.
   * Used only during onboarding/migration — not for daily unlock.
   * The userId serves as a deterministic, per-user salt.
   */
  async deriveWrappingKey(password: string, userId: string): Promise<CryptoKey> {
    const salt = stringToArrayBuffer(userId);
    return this.deriveKey({
      password,
      salt: new Uint8Array(salt),
      iterations: CRYPTO_WRAPPING_ITERATIONS,
      extractable: false,
    });
  }

  /**
   * Wrap (encrypt) a master key with a wrapping key using AES-GCM
   */
  async wrapMasterKey(wrappingKey: CryptoKey, masterKeyBytes: Uint8Array): Promise<EncryptionResult> {
    const iv = this.generateIV();
    const ciphertext = await crypto.subtle.encrypt(
      { name: ALGORITHM, iv: iv as BufferSource },
      wrappingKey,
      masterKeyBytes as Uint8Array<ArrayBuffer>
    );
    return {
      ciphertext: arrayBufferToBase64(ciphertext),
      iv: arrayBufferToBase64(iv),
    };
  }

  /**
   * Unwrap (decrypt) a master key from a wrapped blob using AES-GCM
   */
  async unwrapMasterKey(wrappingKey: CryptoKey, wrappedBlob: EncryptionResult): Promise<Uint8Array> {
    try {
      const decrypted = await crypto.subtle.decrypt(
        { name: ALGORITHM, iv: base64ToArrayBuffer(wrappedBlob.iv) },
        wrappingKey,
        base64ToArrayBuffer(wrappedBlob.ciphertext)
      );
      return new Uint8Array(decrypted);
    } catch {
      throw new Error('Failed to unwrap master key. Wrong password or corrupted data.');
    }
  }

  /**
   * Generate UUID v4
   */
  generateUUID(): string {
    return crypto.randomUUID();
  }
}

/**
 * Singleton instance of crypto service
 */
export const cryptoService = new WebCryptoService();

/**
 * Helper function to encrypt JSON data
 */
export async function encryptJSON(
  data: unknown,
  key: CryptoKey
): Promise<EncryptionResult> {
  const json = JSON.stringify(data);
  return cryptoService.encryptText(json, key);
}

/**
 * Helper function to decrypt JSON data
 */
export async function decryptJSON<T>(
  params: DecryptionParams,
  key: CryptoKey
): Promise<T> {
  const json = await cryptoService.decryptText(params, key);
  return JSON.parse(json);
}

/**
 * Helper to encrypt an array of strings (for tags)
 */
export async function encryptStringArray(
  strings: string[],
  key: CryptoKey
): Promise<EncryptionResult> {
  return encryptJSON(strings, key);
}

/**
 * Helper to decrypt an array of strings (for tags)
 */
export async function decryptStringArray(
  params: DecryptionParams,
  key: CryptoKey
): Promise<string[]> {
  return decryptJSON<string[]>(params, key);
}

/**
 * Update the hash chain with a new content hash.
 * Prepends the current hash and trims to max length.
 *
 * @param currentHash - The new content hash to add
 * @param parentChain - The previous hash chain (or empty array for new notes)
 * @returns Updated hash chain with current hash prepended
 */
export function updateHashChain(
  currentHash: string,
  parentChain: string[] = []
): string[] {
  const newChain = [currentHash, ...parentChain];
  return newChain.slice(0, MAX_HASH_CHAIN_LENGTH);
}

/**
 * Find the common ancestor hash between two hash chains.
 * Returns the first hash that appears in both chains.
 *
 * @param chainA - First hash chain
 * @param chainB - Second hash chain
 * @returns The common ancestor hash, or null if no common ancestor found
 */
export function findCommonAncestor(
  chainA: string[],
  chainB: string[]
): string | null {
  // Create a Set for O(1) lookup
  const chainBSet = new Set(chainB);

  // Find first hash from chainA that exists in chainB
  for (const hash of chainA) {
    if (chainBSet.has(hash)) {
      return hash;
    }
  }

  return null;
}
