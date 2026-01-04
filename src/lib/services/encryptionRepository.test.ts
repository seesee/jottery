/**
 * Tests for encryptionRepository
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { encryptionRepository } from './encryptionRepository';
import type { EncryptionMetadata } from '../types';
import { initTestDB, cleanupTestDB } from '../../test/db-utils';

describe('encryptionRepository', () => {
  beforeEach(async () => {
    await initTestDB();
  });

  afterEach(async () => {
    await cleanupTestDB();
  });

  const createTestMetadata = (): EncryptionMetadata => ({
    salt: 'dGVzdC1zYWx0LWJhc2U2NC1lbmNvZGVk',
    iterations: 100000,
    createdAt: new Date().toISOString(),
    algorithm: 'AES-256-GCM',
  });

  describe('getMetadata', () => {
    it('should return null when no metadata exists', async () => {
      // Delete metadata created by initTestDB
      await encryptionRepository.deleteMetadata();

      const metadata = await encryptionRepository.getMetadata();
      expect(metadata).toBeNull();
    });

    it('should return stored metadata when it exists', async () => {
      const testMetadata = createTestMetadata();
      await encryptionRepository.setMetadata(testMetadata);

      const retrieved = await encryptionRepository.getMetadata();
      expect(retrieved).toEqual(testMetadata);
    });

    it('should preserve all metadata fields', async () => {
      const testMetadata = createTestMetadata();
      await encryptionRepository.setMetadata(testMetadata);

      const retrieved = await encryptionRepository.getMetadata();
      expect(retrieved?.salt).toBe(testMetadata.salt);
      expect(retrieved?.iterations).toBe(testMetadata.iterations);
      expect(retrieved?.createdAt).toBe(testMetadata.createdAt);
      expect(retrieved?.algorithm).toBe(testMetadata.algorithm);
    });

    it('should handle different salt values', async () => {
      const salts = [
        'c2hvcnQtc2FsdA==',
        'bG9uZ2VyLXNhbHQtdmFsdWUtZm9yLXRlc3Rpbmc=',
        'YW5vdGhlci1yYW5kb20tc2FsdC12YWx1ZQ==',
      ];

      for (const salt of salts) {
        const metadata = { ...createTestMetadata(), salt };
        await encryptionRepository.setMetadata(metadata);

        const retrieved = await encryptionRepository.getMetadata();
        expect(retrieved?.salt).toBe(salt);
      }
    });

    it('should handle different iteration counts', async () => {
      const iterationCounts = [100000, 200000, 500000, 1000000];

      for (const iterations of iterationCounts) {
        const metadata = { ...createTestMetadata(), iterations };
        await encryptionRepository.setMetadata(metadata);

        const retrieved = await encryptionRepository.getMetadata();
        expect(retrieved?.iterations).toBe(iterations);
      }
    });

    it('should persist metadata across get calls', async () => {
      const testMetadata = createTestMetadata();
      await encryptionRepository.setMetadata(testMetadata);

      const retrieved1 = await encryptionRepository.getMetadata();
      const retrieved2 = await encryptionRepository.getMetadata();
      const retrieved3 = await encryptionRepository.getMetadata();

      expect(retrieved1).toEqual(testMetadata);
      expect(retrieved2).toEqual(testMetadata);
      expect(retrieved3).toEqual(testMetadata);
    });
  });

  describe('setMetadata', () => {
    it('should store metadata correctly', async () => {
      const testMetadata = createTestMetadata();
      await encryptionRepository.setMetadata(testMetadata);

      const retrieved = await encryptionRepository.getMetadata();
      expect(retrieved).toEqual(testMetadata);
    });

    it('should update existing metadata', async () => {
      const original = createTestMetadata();
      await encryptionRepository.setMetadata(original);

      const updated = {
        ...original,
        salt: 'dXBkYXRlZC1zYWx0LXZhbHVl',
        iterations: 200000,
      };
      await encryptionRepository.setMetadata(updated);

      const retrieved = await encryptionRepository.getMetadata();
      expect(retrieved?.salt).toBe(updated.salt);
      expect(retrieved?.iterations).toBe(updated.iterations);
    });

    it('should handle metadata with minimum iterations', async () => {
      const metadata = { ...createTestMetadata(), iterations: 1 };
      await encryptionRepository.setMetadata(metadata);

      const retrieved = await encryptionRepository.getMetadata();
      expect(retrieved?.iterations).toBe(1);
    });

    it('should handle metadata with high iterations', async () => {
      const metadata = { ...createTestMetadata(), iterations: 10000000 };
      await encryptionRepository.setMetadata(metadata);

      const retrieved = await encryptionRepository.getMetadata();
      expect(retrieved?.iterations).toBe(10000000);
    });

    it('should handle very long salt values', async () => {
      const longSalt = 'A'.repeat(1000); // Very long salt
      const metadata = { ...createTestMetadata(), salt: longSalt };
      await encryptionRepository.setMetadata(metadata);

      const retrieved = await encryptionRepository.getMetadata();
      expect(retrieved?.salt).toBe(longSalt);
    });

    it('should preserve ISO 8601 timestamp format', async () => {
      const timestamps = [
        '2024-01-01T00:00:00.000Z',
        '2024-06-15T12:30:45.123Z',
        '2024-12-31T23:59:59.999Z',
      ];

      for (const createdAt of timestamps) {
        const metadata = { ...createTestMetadata(), createdAt };
        await encryptionRepository.setMetadata(metadata);

        const retrieved = await encryptionRepository.getMetadata();
        expect(retrieved?.createdAt).toBe(createdAt);
      }
    });

    it('should overwrite existing metadata completely', async () => {
      const original = createTestMetadata();
      await encryptionRepository.setMetadata(original);

      const newMetadata: EncryptionMetadata = {
        salt: 'bmV3LXNhbHQ=',
        iterations: 300000,
        createdAt: '2025-01-01T00:00:00.000Z',
        algorithm: 'AES-256-GCM',
      };
      await encryptionRepository.setMetadata(newMetadata);

      const retrieved = await encryptionRepository.getMetadata();
      expect(retrieved).toEqual(newMetadata);
      expect(retrieved).not.toEqual(original);
    });
  });

  describe('isInitialized', () => {
    it('should return false when no metadata exists', async () => {
      // Delete metadata created by initTestDB
      await encryptionRepository.deleteMetadata();

      const initialized = await encryptionRepository.isInitialized();
      expect(initialized).toBe(false);
    });

    it('should return true after setting metadata', async () => {
      const testMetadata = createTestMetadata();
      await encryptionRepository.setMetadata(testMetadata);

      const initialized = await encryptionRepository.isInitialized();
      expect(initialized).toBe(true);
    });

    it('should return false after deleting metadata', async () => {
      const testMetadata = createTestMetadata();
      await encryptionRepository.setMetadata(testMetadata);
      await encryptionRepository.deleteMetadata();

      const initialized = await encryptionRepository.isInitialized();
      expect(initialized).toBe(false);
    });

    it('should be consistent across multiple calls', async () => {
      const testMetadata = createTestMetadata();
      await encryptionRepository.setMetadata(testMetadata);

      const initialized1 = await encryptionRepository.isInitialized();
      const initialized2 = await encryptionRepository.isInitialized();
      const initialized3 = await encryptionRepository.isInitialized();

      expect(initialized1).toBe(true);
      expect(initialized2).toBe(true);
      expect(initialized3).toBe(true);
    });
  });

  describe('deleteMetadata', () => {
    it('should delete existing metadata', async () => {
      const testMetadata = createTestMetadata();
      await encryptionRepository.setMetadata(testMetadata);

      await encryptionRepository.deleteMetadata();

      const metadata = await encryptionRepository.getMetadata();
      expect(metadata).toBeNull();
    });

    it('should not throw when deleting non-existent metadata', async () => {
      await expect(encryptionRepository.deleteMetadata()).resolves.not.toThrow();
    });

    it('should allow re-initialization after deletion', async () => {
      const original = createTestMetadata();
      await encryptionRepository.setMetadata(original);
      await encryptionRepository.deleteMetadata();

      const newMetadata = {
        ...createTestMetadata(),
        salt: 'bmV3LXNhbHQtYWZ0ZXItZGVsZXRl',
      };
      await encryptionRepository.setMetadata(newMetadata);

      const retrieved = await encryptionRepository.getMetadata();
      expect(retrieved).toEqual(newMetadata);
    });

    it('should be idempotent', async () => {
      const testMetadata = createTestMetadata();
      await encryptionRepository.setMetadata(testMetadata);

      await encryptionRepository.deleteMetadata();
      await encryptionRepository.deleteMetadata();
      await encryptionRepository.deleteMetadata();

      const metadata = await encryptionRepository.getMetadata();
      expect(metadata).toBeNull();
    });
  });

  describe('edge cases', () => {
    it('should handle rapid successive operations', async () => {
      const metadata1 = { ...createTestMetadata(), salt: 'c2FsdDE=' };
      const metadata2 = { ...createTestMetadata(), salt: 'c2FsdDI=' };
      const metadata3 = { ...createTestMetadata(), salt: 'c2FsdDM=' };

      await Promise.all([
        encryptionRepository.setMetadata(metadata1),
        encryptionRepository.setMetadata(metadata2),
        encryptionRepository.setMetadata(metadata3),
      ]);

      const retrieved = await encryptionRepository.getMetadata();
      expect(retrieved).toBeDefined();
      expect(retrieved?.algorithm).toBe('AES-256-GCM');
    });

    it('should handle empty salt string', async () => {
      const metadata = { ...createTestMetadata(), salt: '' };
      await encryptionRepository.setMetadata(metadata);

      const retrieved = await encryptionRepository.getMetadata();
      expect(retrieved?.salt).toBe('');
    });

    it('should handle zero iterations', async () => {
      const metadata = { ...createTestMetadata(), iterations: 0 };
      await encryptionRepository.setMetadata(metadata);

      const retrieved = await encryptionRepository.getMetadata();
      expect(retrieved?.iterations).toBe(0);
    });

    it('should handle special characters in salt', async () => {
      const salts = [
        'dGVzdCt3aXRoK3BsdXM=',
        'dGVzdC93aXRoL3NsYXNo',
        'dGVzdD13aXRoPWVxdWFs',
      ];

      for (const salt of salts) {
        const metadata = { ...createTestMetadata(), salt };
        await encryptionRepository.setMetadata(metadata);

        const retrieved = await encryptionRepository.getMetadata();
        expect(retrieved?.salt).toBe(salt);
      }
    });
  });

  describe('integration scenarios', () => {
    it('should support full encryption setup workflow', async () => {
      // Delete metadata created by initTestDB
      await encryptionRepository.deleteMetadata();

      // Check not initialized
      let initialized = await encryptionRepository.isInitialized();
      expect(initialized).toBe(false);

      // Check no metadata
      let metadata = await encryptionRepository.getMetadata();
      expect(metadata).toBeNull();

      // Set metadata (first-time setup)
      const setupMetadata = createTestMetadata();
      await encryptionRepository.setMetadata(setupMetadata);

      // Verify initialized
      initialized = await encryptionRepository.isInitialized();
      expect(initialized).toBe(true);

      // Verify metadata stored
      metadata = await encryptionRepository.getMetadata();
      expect(metadata).toEqual(setupMetadata);
    });

    it('should support password change workflow', async () => {
      // Original setup
      const original = createTestMetadata();
      await encryptionRepository.setMetadata(original);

      // Simulate password change (new salt, possibly new iterations)
      const afterChange = {
        ...createTestMetadata(),
        salt: 'bmV3LXBhc3N3b3JkLXNhbHQ=',
        iterations: 200000,
        createdAt: new Date().toISOString(),
      };
      await encryptionRepository.setMetadata(afterChange);

      const metadata = await encryptionRepository.getMetadata();
      expect(metadata?.salt).toBe(afterChange.salt);
      expect(metadata?.iterations).toBe(afterChange.iterations);
      expect(metadata?.salt).not.toBe(original.salt);
    });

    it('should support factory reset workflow', async () => {
      // Set up encryption
      const testMetadata = createTestMetadata();
      await encryptionRepository.setMetadata(testMetadata);

      // Verify setup
      let initialized = await encryptionRepository.isInitialized();
      expect(initialized).toBe(true);

      // Factory reset (delete metadata)
      await encryptionRepository.deleteMetadata();

      // Verify reset
      initialized = await encryptionRepository.isInitialized();
      expect(initialized).toBe(false);

      const metadata = await encryptionRepository.getMetadata();
      expect(metadata).toBeNull();
    });

    it('should support iteration upgrade workflow', async () => {
      // Original setup with lower iterations
      const original = { ...createTestMetadata(), iterations: 100000 };
      await encryptionRepository.setMetadata(original);

      // Upgrade iterations (security improvement)
      const upgraded = { ...original, iterations: 500000 };
      await encryptionRepository.setMetadata(upgraded);

      const metadata = await encryptionRepository.getMetadata();
      expect(metadata?.iterations).toBe(500000);
      expect(metadata?.salt).toBe(original.salt); // Salt unchanged
      expect(metadata?.createdAt).toBe(original.createdAt); // Timestamp unchanged
    });

    it('should maintain data integrity across operations', async () => {
      const testMetadata = createTestMetadata();

      // Set
      await encryptionRepository.setMetadata(testMetadata);
      let retrieved = await encryptionRepository.getMetadata();
      expect(retrieved).toEqual(testMetadata);

      // Check initialized
      const initialized = await encryptionRepository.isInitialized();
      expect(initialized).toBe(true);

      // Get again
      retrieved = await encryptionRepository.getMetadata();
      expect(retrieved).toEqual(testMetadata);

      // Delete
      await encryptionRepository.deleteMetadata();
      retrieved = await encryptionRepository.getMetadata();
      expect(retrieved).toBeNull();
    });
  });

  describe('metadata validation', () => {
    it('should store metadata with exact field values', async () => {
      const metadata: EncryptionMetadata = {
        salt: 'dGVzdC1zYWx0',
        iterations: 123456,
        createdAt: '2024-06-15T10:30:00.000Z',
        algorithm: 'AES-256-GCM',
      };

      await encryptionRepository.setMetadata(metadata);
      const retrieved = await encryptionRepository.getMetadata();

      expect(retrieved?.salt).toBe('dGVzdC1zYWx0');
      expect(retrieved?.iterations).toBe(123456);
      expect(retrieved?.createdAt).toBe('2024-06-15T10:30:00.000Z');
      expect(retrieved?.algorithm).toBe('AES-256-GCM');
    });

    it('should preserve algorithm field', async () => {
      const metadata = createTestMetadata();
      await encryptionRepository.setMetadata(metadata);

      const retrieved = await encryptionRepository.getMetadata();
      expect(retrieved?.algorithm).toBe('AES-256-GCM');
    });

    it('should handle metadata with all supported algorithms', async () => {
      // Currently only AES-256-GCM, but test for future extensibility
      const metadata: EncryptionMetadata = {
        salt: 'dGVzdC1zYWx0',
        iterations: 100000,
        createdAt: new Date().toISOString(),
        algorithm: 'AES-256-GCM',
      };

      await encryptionRepository.setMetadata(metadata);
      const retrieved = await encryptionRepository.getMetadata();

      expect(retrieved?.algorithm).toBe('AES-256-GCM');
    });
  });
});
