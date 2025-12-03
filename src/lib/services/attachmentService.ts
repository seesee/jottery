/**
 * Attachment service for handling file attachments with encryption
 * Manages file upload, encryption, storage, thumbnails, and retrieval
 */

import type { Attachment } from '../types';
import { cryptoService } from './crypto';
import { keyManager } from './keyManager';
import { attachmentRepository } from './attachmentRepository';

// Maximum file size: 10MB
const MAX_FILE_SIZE = 10 * 1024 * 1024;

// Thumbnail settings
const THUMBNAIL_MAX_WIDTH = 200;
const THUMBNAIL_MAX_HEIGHT = 200;
const THUMBNAIL_QUALITY = 0.8;

// Image MIME types that support thumbnails
const THUMBNAIL_MIME_TYPES = [
  'image/jpeg',
  'image/jpg',
  'image/png',
  'image/gif',
  'image/webp',
  'image/bmp',
];

/**
 * Read file as ArrayBuffer
 */
async function readFileAsArrayBuffer(file: File): Promise<ArrayBuffer> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      if (reader.result instanceof ArrayBuffer) {
        resolve(reader.result);
      } else {
        reject(new Error('Failed to read file as ArrayBuffer'));
      }
    };
    reader.onerror = () => reject(reader.error);
    reader.readAsArrayBuffer(file);
  });
}

/**
 * Generate thumbnail for image file
 */
async function generateThumbnail(
  file: File
): Promise<ArrayBuffer | null> {
  if (!THUMBNAIL_MIME_TYPES.includes(file.type)) {
    return null;
  }

  return new Promise((resolve) => {
    const img = new Image();
    const url = URL.createObjectURL(file);

    img.onload = () => {
      URL.revokeObjectURL(url);

      // Calculate thumbnail dimensions maintaining aspect ratio
      let width = img.width;
      let height = img.height;

      if (width > height) {
        if (width > THUMBNAIL_MAX_WIDTH) {
          height = (height * THUMBNAIL_MAX_WIDTH) / width;
          width = THUMBNAIL_MAX_WIDTH;
        }
      } else {
        if (height > THUMBNAIL_MAX_HEIGHT) {
          width = (width * THUMBNAIL_MAX_HEIGHT) / height;
          height = THUMBNAIL_MAX_HEIGHT;
        }
      }

      // Create canvas and draw scaled image
      const canvas = document.createElement('canvas');
      canvas.width = width;
      canvas.height = height;
      const ctx = canvas.getContext('2d');

      if (!ctx) {
        resolve(null);
        return;
      }

      ctx.drawImage(img, 0, 0, width, height);

      // Convert to blob
      canvas.toBlob(
        async (blob) => {
          if (!blob) {
            resolve(null);
            return;
          }

          const arrayBuffer = await blob.arrayBuffer();
          resolve(arrayBuffer);
        },
        'image/jpeg',
        THUMBNAIL_QUALITY
      );
    };

    img.onerror = () => {
      URL.revokeObjectURL(url);
      resolve(null);
    };

    img.src = url;
  });
}

class AttachmentService {
  /**
   * Add attachment from file
   */
  async addAttachment(file: File): Promise<Attachment> {
    // Validate file size
    if (file.size > MAX_FILE_SIZE) {
      throw new Error(
        `File size exceeds maximum allowed size of ${MAX_FILE_SIZE / 1024 / 1024}MB`
      );
    }

    // Get encryption key
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked. Unlock to add attachments.');
    }

    // Generate attachment ID
    const id = cryptoService.generateUUID();

    // Read file data
    const fileData = await readFileAsArrayBuffer(file);

    // Encrypt file data
    const encryptedData = await cryptoService.encryptBinary(
      fileData,
      masterKey.key
    );

    // Store encrypted file data
    const dataBlob = new TextEncoder().encode(
      JSON.stringify(encryptedData)
    ).buffer;
    await attachmentRepository.storeBlob(id, dataBlob);

    // Generate and store thumbnail if it's an image
    let thumbnailStored = false;
    if (THUMBNAIL_MIME_TYPES.includes(file.type)) {
      try {
        const thumbnailData = await generateThumbnail(file);
        if (thumbnailData) {
          const encryptedThumbnail = await cryptoService.encryptBinary(
            thumbnailData,
            masterKey.key
          );
          const thumbnailBlob = new TextEncoder().encode(
            JSON.stringify(encryptedThumbnail)
          ).buffer;
          await attachmentRepository.storeThumbnail(id, thumbnailBlob);
          thumbnailStored = true;
        }
      } catch (error) {
        console.warn('Failed to generate thumbnail:', error);
        // Continue without thumbnail
      }
    }

    // Encrypt filename
    const encryptedFilename = await cryptoService.encryptText(
      file.name,
      masterKey.key
    );

    // Create attachment metadata
    const attachment: Attachment = {
      id,
      filename: JSON.stringify(encryptedFilename),
      mimeType: file.type,
      size: file.size,
      data: id, // Reference to blob storage
      thumbnailData: thumbnailStored ? id : undefined,
    };

    return attachment;
  }

  /**
   * Get attachment blob data (decrypted)
   */
  async getAttachmentData(attachment: Attachment): Promise<Blob> {
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked. Unlock to view attachments.');
    }

    // Retrieve encrypted data
    const dataBlob = await attachmentRepository.getBlob(attachment.data);
    if (!dataBlob) {
      throw new Error('Attachment data not found');
    }

    // Parse encrypted data
    const encryptedDataJson = new TextDecoder().decode(dataBlob);
    const encryptedData = JSON.parse(encryptedDataJson);

    // Decrypt data
    const decryptedData = await cryptoService.decryptBinary(
      encryptedData,
      masterKey.key
    );

    // Get decrypted filename
    const filename = await this.getDecryptedFilename(attachment);

    // Create blob
    return new Blob([decryptedData], { type: attachment.mimeType });
  }

  /**
   * Get thumbnail data (decrypted)
   */
  async getThumbnailData(attachment: Attachment): Promise<string | null> {
    if (!attachment.thumbnailData) {
      return null;
    }

    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      return null;
    }

    try {
      // Retrieve encrypted thumbnail
      const thumbnailBlob = await attachmentRepository.getThumbnail(
        attachment.thumbnailData
      );
      if (!thumbnailBlob) {
        return null;
      }

      // Parse encrypted thumbnail
      const encryptedThumbnailJson = new TextDecoder().decode(thumbnailBlob);
      const encryptedThumbnail = JSON.parse(encryptedThumbnailJson);

      // Decrypt thumbnail
      const decryptedThumbnail = await cryptoService.decryptBinary(
        encryptedThumbnail,
        masterKey.key
      );

      // Convert to data URL
      const blob = new Blob([decryptedThumbnail], { type: 'image/jpeg' });
      return URL.createObjectURL(blob);
    } catch (error) {
      console.error('Failed to load thumbnail:', error);
      return null;
    }
  }

  /**
   * Get decrypted filename
   */
  async getDecryptedFilename(attachment: Attachment): Promise<string> {
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      return 'attachment'; // Fallback when locked
    }

    try {
      const encryptedFilename = JSON.parse(attachment.filename);
      return await cryptoService.decryptText(
        encryptedFilename,
        masterKey.key
      );
    } catch (error) {
      console.error('Failed to decrypt filename:', error);
      return 'attachment';
    }
  }

  /**
   * Download attachment
   */
  async downloadAttachment(attachment: Attachment): Promise<void> {
    const blob = await this.getAttachmentData(attachment);
    const filename = await this.getDecryptedFilename(attachment);

    // Create download link
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  }

  /**
   * Delete attachment (from storage)
   */
  async deleteAttachment(attachment: Attachment): Promise<void> {
    // Delete blob data
    await attachmentRepository.deleteBlob(attachment.data);

    // Delete thumbnail if exists
    if (attachment.thumbnailData) {
      await attachmentRepository.deleteThumbnail(attachment.thumbnailData);
    }
  }

  /**
   * Delete multiple attachments
   */
  async deleteAttachments(attachments: Attachment[]): Promise<void> {
    await Promise.all(
      attachments.map((attachment) => this.deleteAttachment(attachment))
    );
  }

  /**
   * Get total storage size of all attachments
   */
  async getTotalStorageSize(): Promise<number> {
    return attachmentRepository.getTotalSize();
  }

  /**
   * Format file size for display
   */
  formatFileSize(bytes: number): string {
    if (bytes === 0) return '0 B';

    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));

    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
  }

  /**
   * Check if file type supports thumbnails
   */
  supportsThumbnail(mimeType: string): boolean {
    return THUMBNAIL_MIME_TYPES.includes(mimeType);
  }

  /**
   * Validate file before upload
   */
  validateFile(file: File): { valid: boolean; error?: string } {
    if (file.size > MAX_FILE_SIZE) {
      return {
        valid: false,
        error: `File size exceeds ${MAX_FILE_SIZE / 1024 / 1024}MB limit`,
      };
    }

    if (file.size === 0) {
      return {
        valid: false,
        error: 'File is empty',
      };
    }

    return { valid: true };
  }
}

/**
 * Singleton instance
 */
export const attachmentService = new AttachmentService();
