/**
 * Cryptography-related types and interfaces
 */

/**
 * Master key derived from user password
 * Never persisted, exists only in memory
 */
export interface MasterKey {
  key: CryptoKey;
  derivedAt: number; // Timestamp when derived
}

/**
 * Key derivation parameters
 */
export interface KeyDerivationParams {
  password: string;
  salt: Uint8Array;
  iterations: number;
  algorithm: 'PBKDF2' | 'Argon2id';
}

/**
 * Encryption result
 */
export interface EncryptionResult {
  ciphertext: string; // Base64 encoded
  iv: string; // Base64 encoded initialization vector
  authTag?: string; // Base64 encoded auth tag (for GCM)
}

/**
 * Decryption parameters
 */
export interface DecryptionParams {
  ciphertext: string;
  iv: string;
  authTag?: string;
}

/**
 * Crypto service interface
 */
export interface CryptoService {
  /**
   * Derive master key from password
   */
  deriveKey(params: KeyDerivationParams): Promise<CryptoKey>;

  /**
   * Encrypt text data
   */
  encryptText(plaintext: string, key: CryptoKey): Promise<EncryptionResult>;

  /**
   * Decrypt text data
   */
  decryptText(params: DecryptionParams, key: CryptoKey): Promise<string>;

  /**
   * Encrypt binary data
   */
  encryptBinary(data: ArrayBuffer, key: CryptoKey): Promise<EncryptionResult>;

  /**
   * Decrypt binary data
   */
  decryptBinary(params: DecryptionParams, key: CryptoKey): Promise<ArrayBuffer>;

  /**
   * Generate random salt
   */
  generateSalt(): Uint8Array;

  /**
   * Generate initialization vector
   */
  generateIV(): Uint8Array;

  /**
   * Hash data (for sync conflict detection)
   */
  hash(data: string): Promise<string>;

  /**
   * Generate UUID v4
   */
  generateUUID(): string;
}

/**
 * Key manager service interface
 */
export interface KeyManager {
  /**
   * Get current master key (if unlocked)
   */
  getMasterKey(): MasterKey | null;

  /**
   * Set master key (on unlock)
   */
  setMasterKey(key: MasterKey): void;

  /**
   * Clear master key (on lock)
   */
  clearMasterKey(): void;

  /**
   * Check if application is locked
   */
  isLocked(): boolean;

  /**
   * Register activity (for auto-lock timeout)
   */
  registerActivity(): void;

  /**
   * Start auto-lock timer
   */
  startAutoLock(timeoutMinutes: number): void;

  /**
   * Stop auto-lock timer
   */
  stopAutoLock(): void;
}

/**
 * Conflict resolver interface for sync (Phase 3)
 */
export interface ConflictResolver<T> {
  /**
   * Resolve conflict between local and remote versions
   */
  resolve(local: T, remote: T): T;
}
