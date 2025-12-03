/**
 * Export/Import service for notes data
 */

import type { ExportData, ExportNote, ExportAttachment, Note, Attachment } from '../types';
import { noteRepository } from './noteRepository';
import { attachmentRepository } from './attachmentRepository';
import { noteService } from './noteService';
import { keyManager } from './keyManager';
import { cryptoService, decryptStringArray } from './crypto';

const EXPORT_VERSION = '1.0';

/**
 * Export all notes to JSON format
 */
export async function exportAllNotes(): Promise<ExportData> {
  const masterKey = keyManager.getMasterKey();
  if (!masterKey) {
    throw new Error('Application is locked');
  }

  const notes = await noteRepository.getAllActive();
  const exportNotes: ExportNote[] = [];

  for (const note of notes) {
    const exportNote = await convertNoteToExport(note, masterKey.key);
    exportNotes.push(exportNote);
  }

  return {
    version: EXPORT_VERSION,
    exportDate: new Date().toISOString(),
    notes: exportNotes,
  };
}

/**
 * Export filtered notes (by IDs)
 */
export async function exportNotes(noteIds: string[]): Promise<ExportData> {
  const masterKey = keyManager.getMasterKey();
  if (!masterKey) {
    throw new Error('Application is locked');
  }

  const notes = await noteRepository.getByIds(noteIds);
  const exportNotes: ExportNote[] = [];

  for (const note of notes) {
    const exportNote = await convertNoteToExport(note, masterKey.key);
    exportNotes.push(exportNote);
  }

  return {
    version: EXPORT_VERSION,
    exportDate: new Date().toISOString(),
    notes: exportNotes,
  };
}

/**
 * Convert a note to export format (decrypt all fields)
 */
async function convertNoteToExport(note: Note, key: CryptoKey): Promise<ExportNote> {
  // Decrypt content
  const encryptedContent = JSON.parse(note.content);
  const content = await cryptoService.decryptText(encryptedContent, key);

  // Decrypt tags
  let tags: string[] = [];
  if (note.tags.length > 0 && note.tags[0]) {
    const encryptedTags = JSON.parse(note.tags[0]);
    tags = await decryptStringArray(encryptedTags, key);
  }

  // Decrypt and export attachments
  const exportAttachments: ExportAttachment[] = [];
  for (const attachment of note.attachments) {
    // Get encrypted blob from storage
    const encryptedBlob = await attachmentRepository.getBlob(attachment.data);
    if (!encryptedBlob) continue;

    // Decrypt attachment filename
    const filenameEncrypted = JSON.parse(attachment.filename);
    const filename = await cryptoService.decryptText(filenameEncrypted, key);

    // Decrypt attachment data
    const encryptedDataJson = new TextDecoder().decode(encryptedBlob);
    const encryptedData = JSON.parse(encryptedDataJson);
    const decryptedData = await cryptoService.decryptBinary(encryptedData, key);

    exportAttachments.push({
      filename,
      mimeType: attachment.mimeType,
      data: arrayBufferToBase64(decryptedData),
    });
  }

  return {
    id: note.id,
    createdAt: note.createdAt,
    modifiedAt: note.modifiedAt,
    content,
    tags,
    attachments: exportAttachments,
    pinned: note.pinned,
    wordWrap: note.wordWrap,
    syntaxLanguage: note.syntaxLanguage,
  };
}

/**
 * Import notes from JSON data
 */
export async function importNotes(
  data: ExportData,
  strategy: 'merge' | 'replace' | 'skip' = 'merge'
): Promise<{
  imported: number;
  skipped: number;
  errors: string[];
}> {
  const masterKey = keyManager.getMasterKey();
  if (!masterKey) {
    throw new Error('Application is locked');
  }

  let imported = 0;
  let skipped = 0;
  const errors: string[] = [];

  // Validate export version
  if (data.version !== EXPORT_VERSION) {
    errors.push(`Unsupported export version: ${data.version}`);
    return { imported, skipped, errors };
  }

  for (const exportNote of data.notes) {
    try {
      // Check if note already exists
      const existingNote = await noteRepository.getById(exportNote.id);

      if (existingNote) {
        if (strategy === 'skip') {
          skipped++;
          continue;
        } else if (strategy === 'replace') {
          // Delete existing note
          await noteRepository.delete(exportNote.id);
        }
        // For 'merge', we'll import with a new ID
      }

      // Process attachments: re-encrypt and store
      const attachments: Attachment[] = [];
      if (exportNote.attachments && exportNote.attachments.length > 0) {
        for (const exportAttachment of exportNote.attachments) {
          try {
            // Generate new attachment ID
            const attachmentId = cryptoService.generateUUID();

            // Convert base64 to ArrayBuffer
            const fileData = base64ToArrayBuffer(exportAttachment.data);

            // Encrypt file data
            const encryptedData = await cryptoService.encryptBinary(
              fileData,
              masterKey.key
            );

            // Store encrypted file data
            const dataBlob = new TextEncoder().encode(
              JSON.stringify(encryptedData)
            ).buffer;
            await attachmentRepository.storeBlob(attachmentId, dataBlob);

            // Encrypt filename
            const encryptedFilename = await cryptoService.encryptText(
              exportAttachment.filename,
              masterKey.key
            );

            // Create attachment metadata
            attachments.push({
              id: attachmentId,
              filename: JSON.stringify(encryptedFilename),
              mimeType: exportAttachment.mimeType,
              size: fileData.byteLength,
              data: attachmentId, // Reference to blob storage
            });
          } catch (attachmentError) {
            console.error('Failed to import attachment:', attachmentError);
            // Continue with other attachments
          }
        }
      }

      // Create new note with imported content, preserving timestamps and settings
      await noteService.createNote(exportNote.content, exportNote.tags, {
        createdAt: exportNote.createdAt,
        modifiedAt: exportNote.modifiedAt,
        pinned: exportNote.pinned,
        wordWrap: exportNote.wordWrap,
        syntaxLanguage: exportNote.syntaxLanguage,
        attachments: attachments,
      });
      imported++;
    } catch (error) {
      errors.push(
        `Failed to import note ${exportNote.id}: ${error instanceof Error ? error.message : 'Unknown error'}`
      );
    }
  }

  return { imported, skipped, errors };
}

/**
 * Download export data as JSON file
 */
export async function downloadExport(data: ExportData, filename?: string): Promise<void> {
  const json = JSON.stringify(data, null, 2);
  const blob = new Blob([json], { type: 'application/json' });
  const url = URL.createObjectURL(blob);

  const a = document.createElement('a');
  a.href = url;
  a.download = filename || `jottery-export-${new Date().toISOString().split('T')[0]}.json`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

/**
 * Parse imported JSON file
 */
export async function parseImportFile(file: File): Promise<ExportData> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();

    reader.onload = (e) => {
      try {
        const text = e.target?.result as string;
        const data = JSON.parse(text);

        // Validate basic structure
        if (!data.version || !data.notes || !Array.isArray(data.notes)) {
          throw new Error('Invalid export file format');
        }

        resolve(data as ExportData);
      } catch (error) {
        reject(new Error('Failed to parse import file'));
      }
    };

    reader.onerror = () => {
      reject(new Error('Failed to read file'));
    };

    reader.readAsText(file);
  });
}

/**
 * Helper: Convert ArrayBuffer to Base64
 */
function arrayBufferToBase64(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = '';
  for (let i = 0; i < bytes.byteLength; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

/**
 * Helper: Convert Base64 to ArrayBuffer
 */
function base64ToArrayBuffer(base64: string): ArrayBuffer {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes.buffer;
}
