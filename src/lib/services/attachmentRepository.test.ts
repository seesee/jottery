/**
 * Comprehensive Attachment Repository tests
 * Tests blob storage for attachments and thumbnails
 */

import { describe, test, expect, beforeEach, afterEach } from 'vitest';
import { attachmentRepository } from './attachmentRepository';
import { initTestDB, cleanupTestDB } from '../../test/db-utils';
import { cryptoService } from './crypto';

describe('AttachmentRepository', () => {
  beforeEach(async () => {
    await initTestDB();
  });

  afterEach(async () => {
    await cleanupTestDB();
  });

  describe('Blob Storage', () => {
    describe('storeBlob and getBlob', () => {
      test('should store and retrieve blob data', async () => {
        const id = cryptoService.generateUUID();
        const data = new Uint8Array([1, 2, 3, 4, 5]).buffer;

        await attachmentRepository.storeBlob(id, data);
        const retrieved = await attachmentRepository.getBlob(id);

        expect(retrieved).toBeDefined();
        expect(new Uint8Array(retrieved!)).toEqual(new Uint8Array(data));
      });

      test('should return null for non-existent blob', async () => {
        const retrieved = await attachmentRepository.getBlob('non-existent-id');

        expect(retrieved).toBeNull();
      });

      test('should handle empty blob', async () => {
        const id = cryptoService.generateUUID();
        const data = new Uint8Array([]).buffer;

        await attachmentRepository.storeBlob(id, data);
        const retrieved = await attachmentRepository.getBlob(id);

        expect(retrieved).toBeDefined();
        expect(retrieved!.byteLength).toBe(0);
      });

      test('should handle large blob (64KB - test env limit)', async () => {
        const id = cryptoService.generateUUID();
        // Note: happy-dom limits crypto.getRandomValues to 64KB
        const data = crypto.getRandomValues(new Uint8Array(64 * 1024)).buffer;

        await attachmentRepository.storeBlob(id, data);
        const retrieved = await attachmentRepository.getBlob(id);

        expect(retrieved).toBeDefined();
        expect(new Uint8Array(retrieved!)).toEqual(new Uint8Array(data));
      });

      test('should overwrite existing blob', async () => {
        const id = cryptoService.generateUUID();
        const data1 = new Uint8Array([1, 2, 3]).buffer;
        const data2 = new Uint8Array([4, 5, 6]).buffer;

        await attachmentRepository.storeBlob(id, data1);
        await attachmentRepository.storeBlob(id, data2);

        const retrieved = await attachmentRepository.getBlob(id);
        expect(new Uint8Array(retrieved!)).toEqual(new Uint8Array(data2));
      });

      test('should handle multiple blobs', async () => {
        const id1 = cryptoService.generateUUID();
        const id2 = cryptoService.generateUUID();
        const data1 = new Uint8Array([1, 2, 3]).buffer;
        const data2 = new Uint8Array([4, 5, 6]).buffer;

        await attachmentRepository.storeBlob(id1, data1);
        await attachmentRepository.storeBlob(id2, data2);

        const retrieved1 = await attachmentRepository.getBlob(id1);
        const retrieved2 = await attachmentRepository.getBlob(id2);

        expect(new Uint8Array(retrieved1!)).toEqual(new Uint8Array(data1));
        expect(new Uint8Array(retrieved2!)).toEqual(new Uint8Array(data2));
      });

      test('should handle binary data with all byte values', async () => {
        const id = cryptoService.generateUUID();
        const data = new Uint8Array(256);
        for (let i = 0; i < 256; i++) {
          data[i] = i;
        }

        await attachmentRepository.storeBlob(id, data.buffer);
        const retrieved = await attachmentRepository.getBlob(id);

        expect(new Uint8Array(retrieved!)).toEqual(data);
      });
    });

    describe('deleteBlob', () => {
      test('should delete blob', async () => {
        const id = cryptoService.generateUUID();
        const data = new Uint8Array([1, 2, 3]).buffer;

        await attachmentRepository.storeBlob(id, data);
        await attachmentRepository.deleteBlob(id);

        const retrieved = await attachmentRepository.getBlob(id);
        expect(retrieved).toBeNull();
      });

      test('should not error when deleting non-existent blob', async () => {
        await expect(attachmentRepository.deleteBlob('non-existent')).resolves.not.toThrow();
      });

      test('should delete only specified blob', async () => {
        const id1 = cryptoService.generateUUID();
        const id2 = cryptoService.generateUUID();
        const data1 = new Uint8Array([1, 2, 3]).buffer;
        const data2 = new Uint8Array([4, 5, 6]).buffer;

        await attachmentRepository.storeBlob(id1, data1);
        await attachmentRepository.storeBlob(id2, data2);
        await attachmentRepository.deleteBlob(id1);

        const retrieved1 = await attachmentRepository.getBlob(id1);
        const retrieved2 = await attachmentRepository.getBlob(id2);

        expect(retrieved1).toBeNull();
        expect(retrieved2).toBeDefined();
      });
    });
  });

  describe('Thumbnail Storage', () => {
    describe('storeThumbnail and getThumbnail', () => {
      test('should store and retrieve thumbnail', async () => {
        const id = cryptoService.generateUUID();
        const data = new Uint8Array([10, 20, 30]).buffer;

        await attachmentRepository.storeThumbnail(id, data);
        const retrieved = await attachmentRepository.getThumbnail(id);

        expect(retrieved).toBeDefined();
        expect(new Uint8Array(retrieved!)).toEqual(new Uint8Array(data));
      });

      test('should return null for non-existent thumbnail', async () => {
        const retrieved = await attachmentRepository.getThumbnail('non-existent-id');

        expect(retrieved).toBeNull();
      });

      test('should handle small thumbnail', async () => {
        const id = cryptoService.generateUUID();
        // Simulating a small thumbnail
        const data = crypto.getRandomValues(new Uint8Array(5 * 1024)).buffer; // 5KB

        await attachmentRepository.storeThumbnail(id, data);
        const retrieved = await attachmentRepository.getThumbnail(id);

        expect(retrieved).toBeDefined();
        expect(new Uint8Array(retrieved!)).toEqual(new Uint8Array(data));
      });

      test('should overwrite existing thumbnail', async () => {
        const id = cryptoService.generateUUID();
        const data1 = new Uint8Array([1, 2]).buffer;
        const data2 = new Uint8Array([3, 4]).buffer;

        await attachmentRepository.storeThumbnail(id, data1);
        await attachmentRepository.storeThumbnail(id, data2);

        const retrieved = await attachmentRepository.getThumbnail(id);
        expect(new Uint8Array(retrieved!)).toEqual(new Uint8Array(data2));
      });
    });

    describe('deleteThumbnail', () => {
      test('should delete thumbnail', async () => {
        const id = cryptoService.generateUUID();
        const data = new Uint8Array([1, 2, 3]).buffer;

        await attachmentRepository.storeThumbnail(id, data);
        await attachmentRepository.deleteThumbnail(id);

        const retrieved = await attachmentRepository.getThumbnail(id);
        expect(retrieved).toBeNull();
      });

      test('should not error when deleting non-existent thumbnail', async () => {
        await expect(attachmentRepository.deleteThumbnail('non-existent')).resolves.not.toThrow();
      });
    });
  });

  describe('Combined Operations', () => {
    describe('deleteAll', () => {
      test('should delete both blob and thumbnail', async () => {
        const id = cryptoService.generateUUID();
        const blobData = new Uint8Array([1, 2, 3]).buffer;
        const thumbnailData = new Uint8Array([4, 5, 6]).buffer;

        await attachmentRepository.storeBlob(id, blobData);
        await attachmentRepository.storeThumbnail(id, thumbnailData);

        await attachmentRepository.deleteAll(id);

        const blob = await attachmentRepository.getBlob(id);
        const thumbnail = await attachmentRepository.getThumbnail(id);

        expect(blob).toBeNull();
        expect(thumbnail).toBeNull();
      });

      test('should handle deletion when only blob exists', async () => {
        const id = cryptoService.generateUUID();
        const data = new Uint8Array([1, 2, 3]).buffer;

        await attachmentRepository.storeBlob(id, data);
        await attachmentRepository.deleteAll(id);

        const blob = await attachmentRepository.getBlob(id);
        expect(blob).toBeNull();
      });

      test('should handle deletion when only thumbnail exists', async () => {
        const id = cryptoService.generateUUID();
        const data = new Uint8Array([1, 2, 3]).buffer;

        await attachmentRepository.storeThumbnail(id, data);
        await attachmentRepository.deleteAll(id);

        const thumbnail = await attachmentRepository.getThumbnail(id);
        expect(thumbnail).toBeNull();
      });

      test('should not error when nothing exists', async () => {
        await expect(attachmentRepository.deleteAll('non-existent')).resolves.not.toThrow();
      });
    });

    test('should store blob and thumbnail with same ID independently', async () => {
      const id = cryptoService.generateUUID();
      const blobData = new Uint8Array([1, 2, 3, 4, 5]).buffer;
      const thumbnailData = new Uint8Array([10, 20, 30]).buffer;

      await attachmentRepository.storeBlob(id, blobData);
      await attachmentRepository.storeThumbnail(id, thumbnailData);

      const blob = await attachmentRepository.getBlob(id);
      const thumbnail = await attachmentRepository.getThumbnail(id);

      expect(new Uint8Array(blob!)).toEqual(new Uint8Array(blobData));
      expect(new Uint8Array(thumbnail!)).toEqual(new Uint8Array(thumbnailData));
    });
  });

  describe('Utility Operations', () => {
    describe('getTotalSize', () => {
      test('should return 0 when no attachments', async () => {
        const totalSize = await attachmentRepository.getTotalSize();

        expect(totalSize).toBe(0);
      });

      test('should calculate total size of single attachment', async () => {
        const id = cryptoService.generateUUID();
        const data = new Uint8Array(1024).buffer; // 1KB

        await attachmentRepository.storeBlob(id, data);

        const totalSize = await attachmentRepository.getTotalSize();
        expect(totalSize).toBe(1024);
      });

      test('should calculate total size of multiple attachments', async () => {
        const id1 = cryptoService.generateUUID();
        const id2 = cryptoService.generateUUID();
        const id3 = cryptoService.generateUUID();
        const data1 = new Uint8Array(500).buffer;
        const data2 = new Uint8Array(1000).buffer;
        const data3 = new Uint8Array(1500).buffer;

        await attachmentRepository.storeBlob(id1, data1);
        await attachmentRepository.storeBlob(id2, data2);
        await attachmentRepository.storeBlob(id3, data3);

        const totalSize = await attachmentRepository.getTotalSize();
        expect(totalSize).toBe(3000);
      });

      test('should not include thumbnails in total size', async () => {
        const id = cryptoService.generateUUID();
        const blobData = new Uint8Array(1000).buffer;
        const thumbnailData = new Uint8Array(500).buffer;

        await attachmentRepository.storeBlob(id, blobData);
        await attachmentRepository.storeThumbnail(id, thumbnailData);

        const totalSize = await attachmentRepository.getTotalSize();
        expect(totalSize).toBe(1000); // Only blob size
      });

      test('should update total size after deletion', async () => {
        const id1 = cryptoService.generateUUID();
        const id2 = cryptoService.generateUUID();
        const data1 = new Uint8Array(1000).buffer;
        const data2 = new Uint8Array(2000).buffer;

        await attachmentRepository.storeBlob(id1, data1);
        await attachmentRepository.storeBlob(id2, data2);

        await attachmentRepository.deleteBlob(id1);

        const totalSize = await attachmentRepository.getTotalSize();
        expect(totalSize).toBe(2000);
      });
    });

    describe('listAllIds', () => {
      test('should return empty array when no attachments', async () => {
        const ids = await attachmentRepository.listAllIds();

        expect(ids).toEqual([]);
      });

      test('should list all attachment IDs', async () => {
        const id1 = cryptoService.generateUUID();
        const id2 = cryptoService.generateUUID();
        const id3 = cryptoService.generateUUID();
        const data = new Uint8Array([1, 2, 3]).buffer;

        await attachmentRepository.storeBlob(id1, data);
        await attachmentRepository.storeBlob(id2, data);
        await attachmentRepository.storeBlob(id3, data);

        const ids = await attachmentRepository.listAllIds();

        expect(ids).toHaveLength(3);
        expect(ids).toContain(id1);
        expect(ids).toContain(id2);
        expect(ids).toContain(id3);
      });

      test('should not include thumbnail-only IDs', async () => {
        const blobId = cryptoService.generateUUID();
        const thumbnailOnlyId = cryptoService.generateUUID();
        const data = new Uint8Array([1, 2, 3]).buffer;

        await attachmentRepository.storeBlob(blobId, data);
        await attachmentRepository.storeThumbnail(thumbnailOnlyId, data);

        const ids = await attachmentRepository.listAllIds();

        expect(ids).toHaveLength(1);
        expect(ids).toContain(blobId);
        expect(ids).not.toContain(thumbnailOnlyId);
      });

      test('should update list after deletion', async () => {
        const id1 = cryptoService.generateUUID();
        const id2 = cryptoService.generateUUID();
        const data = new Uint8Array([1, 2, 3]).buffer;

        await attachmentRepository.storeBlob(id1, data);
        await attachmentRepository.storeBlob(id2, data);

        await attachmentRepository.deleteBlob(id1);

        const ids = await attachmentRepository.listAllIds();

        expect(ids).toHaveLength(1);
        expect(ids).toContain(id2);
        expect(ids).not.toContain(id1);
      });
    });
  });

  describe('Edge Cases', () => {
    test('should handle rapid sequential operations', async () => {
      const id = cryptoService.generateUUID();
      const data1 = new Uint8Array([1]).buffer;
      const data2 = new Uint8Array([2]).buffer;
      const data3 = new Uint8Array([3]).buffer;

      await attachmentRepository.storeBlob(id, data1);
      await attachmentRepository.storeBlob(id, data2);
      await attachmentRepository.storeBlob(id, data3);

      const retrieved = await attachmentRepository.getBlob(id);
      expect(new Uint8Array(retrieved!)).toEqual(new Uint8Array(data3));
    });

    test('should handle concurrent operations', async () => {
      const id1 = cryptoService.generateUUID();
      const id2 = cryptoService.generateUUID();
      const data1 = new Uint8Array([1, 2, 3]).buffer;
      const data2 = new Uint8Array([4, 5, 6]).buffer;

      await Promise.all([
        attachmentRepository.storeBlob(id1, data1),
        attachmentRepository.storeBlob(id2, data2),
      ]);

      const [retrieved1, retrieved2] = await Promise.all([
        attachmentRepository.getBlob(id1),
        attachmentRepository.getBlob(id2),
      ]);

      expect(new Uint8Array(retrieved1!)).toEqual(new Uint8Array(data1));
      expect(new Uint8Array(retrieved2!)).toEqual(new Uint8Array(data2));
    });

    test('should handle many attachments', async () => {
      const count = 50;
      const ids: string[] = [];
      const data = new Uint8Array([1, 2, 3]).buffer;

      // Store many attachments
      for (let i = 0; i < count; i++) {
        const id = cryptoService.generateUUID();
        ids.push(id);
        await attachmentRepository.storeBlob(id, data);
      }

      // Verify all stored
      const allIds = await attachmentRepository.listAllIds();
      expect(allIds).toHaveLength(count);

      // Verify total size
      const totalSize = await attachmentRepository.getTotalSize();
      expect(totalSize).toBe(count * 3);
    });

    test('should handle special characters in IDs', async () => {
      // UUIDs should not have special chars, but test robustness
      const id = 'test-id-with-dashes-123';
      const data = new Uint8Array([1, 2, 3]).buffer;

      await attachmentRepository.storeBlob(id, data);
      const retrieved = await attachmentRepository.getBlob(id);

      expect(retrieved).toBeDefined();
    });
  });

  describe('Integration with Note Attachments', () => {
    test('should support typical attachment workflow', async () => {
      const attachmentId = cryptoService.generateUUID();

      // Simulate file upload
      const fileData = new Uint8Array([10, 20, 30, 40, 50]).buffer;
      const thumbnailData = new Uint8Array([1, 2, 3]).buffer;

      // Store both
      await attachmentRepository.storeBlob(attachmentId, fileData);
      await attachmentRepository.storeThumbnail(attachmentId, thumbnailData);

      // Retrieve for display
      const thumbnail = await attachmentRepository.getThumbnail(attachmentId);
      expect(thumbnail).toBeDefined();

      // Retrieve full file when needed
      const fullFile = await attachmentRepository.getBlob(attachmentId);
      expect(fullFile).toBeDefined();
      expect(fullFile!.byteLength).toBe(5);

      // Cleanup when note is deleted
      await attachmentRepository.deleteAll(attachmentId);

      const afterDelete = await attachmentRepository.getBlob(attachmentId);
      expect(afterDelete).toBeNull();
    });
  });
});
