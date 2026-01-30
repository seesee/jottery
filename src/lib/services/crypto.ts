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
import { arrayBufferToBase64, base64ToArrayBuffer } from '../utils/base64';
import { CRYPTO_PBKDF2_ITERATIONS } from '../constants';

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
    const { password, salt, iterations } = params;

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
      false, // Not extractable for security
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
