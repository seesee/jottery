/**
 * Test utilities for database operations
 */

import { initDB, STORES } from '../lib/services/db';
import { cryptoService } from '../lib/services/crypto';
import type { Note, EncryptionMetadata } from '../lib/types';
import { deleteDB } from 'idb';

const TEST_DB_NAME = 'jottery';

/**
 * Initialize test database with encryption metadata
 */
export async function initTestDB(password: string = 'test-password'): Promise<{
  masterKey: CryptoKey;
  salt: Uint8Array;
}> {
  // Generate encryption metadata
  const salt = cryptoService.generateSalt();
  const masterKey = await cryptoService.deriveKey({
    password,
    salt,
    iterations: 100000,
  });

  // Initialize database
  const db = await initDB();

  // Store encryption metadata
  const metadata: EncryptionMetadata = {
    salt: btoa(String.fromCharCode(...salt)),
    iterations: 100000,
    createdAt: new Date().toISOString(),
    algorithm: 'AES-256-GCM',
  };
  await db.put(STORES.ENCRYPTION, metadata, 'metadata');

  return { masterKey, salt };
}

/**
 * Clean up test database
 */
export async function cleanupTestDB(): Promise<void> {
  try {
    await deleteDB(TEST_DB_NAME);
  } catch (error) {
    console.error('Error cleaning up test database:', error);
  }
}

/**
 * Create a test note with optional overrides
 */
export function createTestNote(overrides: Partial<Note> = {}): Note {
  const now = new Date().toISOString();
  return {
    id: cryptoService.generateUUID(),
    createdAt: now,
    modifiedAt: now,
    content: 'Test note content',
    tags: ['test'],
    attachments: [],
    pinned: false,
    deleted: false,
    version: 0,
    wordWrap: true,
    syntaxLanguage: 'plain',
    ...overrides,
  };
}

/**
 * Create multiple test notes
 */
export function createTestNotes(count: number, baseOverrides: Partial<Note> = {}): Note[] {
  return Array.from({ length: count }, (_, i) =>
    createTestNote({
      content: `Test note ${i + 1}`,
      tags: [`tag${i % 3}`],
      ...baseOverrides,
    })
  );
}

/**
 * Wait for a specified time (for async operations)
 */
export function wait(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}
