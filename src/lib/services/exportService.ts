/**
 * Export/Import service for notes data
 */

import type { ExportData, ExportNote, ExportAttachment, Note, Attachment } from '../types';
import { noteRepository } from './noteRepository';
import { attachmentRepository } from './attachmentRepository';
import { noteService } from './noteService';
import { keyManager } from './keyManager';
import { cryptoService, decryptStringArray } from './crypto';
import { arrayBufferToBase64, base64ToArrayBuffer } from '../utils/base64';

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

  // Export color settings
  const { settingsRepository } = await import('./settingsRepository');
  const userSettings = await settingsRepository.get();

  return {
    version: EXPORT_VERSION,
    exportDate: new Date().toISOString(),
    notes: exportNotes,
    settings: {
      colorPalette: userSettings.colorPalette,
      tagColors: userSettings.tagColors,
    },
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

  // Export color settings
  const { settingsRepository } = await import('./settingsRepository');
  const userSettings = await settingsRepository.get();

  return {
    version: EXPORT_VERSION,
    exportDate: new Date().toISOString(),
    notes: exportNotes,
    settings: {
      colorPalette: userSettings.colorPalette,
      tagColors: userSettings.tagColors,
    },
  };
}

/**
 * Convert a note to export format (decrypt all fields)
 */
async function convertNoteToExport(note: Note, key: CryptoKey): Promise<ExportNote> {
  // Decrypt content
  let content: string;
  try {
    const encryptedContent = JSON.parse(note.content);
    content = await cryptoService.decryptText(encryptedContent, key);
  } catch (parseError) {
    console.warn(`Note ${note.id} has unencrypted content, using as-is`);
    content = note.content;
  }

  // Decrypt tags
  let tags: string[] = [];
  if (note.tags.length > 0 && note.tags[0]) {
    try {
      const encryptedTags = JSON.parse(note.tags[0]);
      tags = await decryptStringArray(encryptedTags, key);
    } catch (parseError) {
      console.warn(`Note ${note.id} has unencrypted tags, using as-is`);
      tags = note.tags;
    }
  }

  // Decrypt and export attachments
  const exportAttachments: ExportAttachment[] = [];
  for (const attachment of note.attachments) {
    try {
      // Get encrypted blob from storage
      const encryptedBlob = await attachmentRepository.getBlob(attachment.data);
      if (!encryptedBlob) continue;

      // Decrypt attachment filename
      // Handle both encrypted (JSON) and plain text filenames for backward compatibility
      let filename: string;
      try {
        const filenameEncrypted = JSON.parse(attachment.filename);
        filename = await cryptoService.decryptText(filenameEncrypted, key);
      } catch (parseError) {
        // If JSON.parse fails, assume it's a plain text filename (old data)
        console.warn(`Attachment ${attachment.id} has unencrypted filename, using as-is`);
        filename = attachment.filename;
      }

      // Decrypt attachment data
      const encryptedDataJson = new TextDecoder().decode(encryptedBlob);
      const encryptedData = JSON.parse(encryptedDataJson);
      const decryptedData = await cryptoService.decryptBinary(encryptedData, key);

      exportAttachments.push({
        filename,
        mimeType: attachment.mimeType,
        data: arrayBufferToBase64(decryptedData),
      });
    } catch (attachmentError) {
      console.error(`Failed to export attachment ${attachment.id}:`, attachmentError);
      // Continue with other attachments instead of failing the entire export
    }
  }

  return {
    id: note.id,
    createdAt: note.createdAt,
    modifiedAt: note.modifiedAt,
    content,
    tags,
    attachments: exportAttachments,
    pinned: note.pinned,
    archived: note.archived,
    locked: note.locked,
    wordWrap: note.wordWrap,
    syntaxLanguage: note.syntaxLanguage,
    showPreview: note.showPreview,
    color: note.color,
  };
}

/**
 * Import notes from JSON data
 */
export async function importNotes(
  data: ExportData,
  strategy: 'merge' | 'replace' | 'skip' = 'merge',
  onProgress?: (current: number, total: number) => void
): Promise<{
  imported: number;
  skipped: number;
  errors: string[];
  attachments: number;
  tags: Set<string>;
}> {
  const masterKey = keyManager.getMasterKey();
  if (!masterKey) {
    throw new Error('Application is locked');
  }

  let imported = 0;
  let skipped = 0;
  let attachments = 0;
  const tags = new Set<string>();
  const errors: string[] = [];

  // Validate export version
  if (data.version !== EXPORT_VERSION) {
    errors.push(`Unsupported export version: ${data.version}`);
    return { imported, skipped, errors, attachments, tags };
  }

  // Import color settings if present
  if (data.settings) {
    const { settingsRepository } = await import('./settingsRepository');
    const currentSettings = await settingsRepository.get();

    // Merge color settings (don't overwrite other settings)
    await settingsRepository.update({
      ...currentSettings,
      colorPalette: data.settings.colorPalette || currentSettings.colorPalette,
      tagColors: data.settings.tagColors || currentSettings.tagColors,
    });
  }

  const totalNotes = data.notes.length;

  for (let i = 0; i < data.notes.length; i++) {
    const exportNote = data.notes[i];

    // Normalize tags to always be an array
    if (!Array.isArray(exportNote.tags)) {
      if (typeof exportNote.tags === 'string') {
        // Handle case where tags is a single string
        exportNote.tags = exportNote.tags ? [exportNote.tags] : [];
      } else {
        exportNote.tags = [];
      }
    }

    try {
      // Check if note already exists by ID
      const existingNote = await noteRepository.getById(exportNote.id);

      // Determine if we should preserve the original ID
      let useOriginalId = true;

      if (existingNote) {
        if (strategy === 'skip') {
          skipped++;
          continue;
        } else if (strategy === 'replace') {
          // Delete existing note, then recreate with same ID
          await noteRepository.delete(exportNote.id);
        } else {
          // For 'merge', import with a new ID to create duplicate
          useOriginalId = false;
        }
      }

      // Process attachments: re-encrypt and store
      const noteAttachments: Attachment[] = [];
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
            noteAttachments.push({
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

      // Create note with imported content, preserving timestamps and settings
      // Preserve original ID unless we're merging (creating intentional duplicate)
      await noteService.createNote(exportNote.content, exportNote.tags, {
        id: useOriginalId ? exportNote.id : undefined,
        createdAt: exportNote.createdAt,
        modifiedAt: exportNote.modifiedAt,
        pinned: exportNote.pinned,
        locked: exportNote.locked,
        wordWrap: exportNote.wordWrap,
        syntaxLanguage: exportNote.syntaxLanguage as 'markdown' | 'javascript' | 'python' | 'json' | 'css' | 'bash' | 'sql' | 'plain' | 'html' | undefined,
        showPreview: exportNote.showPreview,
        color: exportNote.color,
        attachments: noteAttachments,
      });

      // Track statistics
      imported++;
      attachments += noteAttachments.length;
      exportNote.tags.forEach(tag => tags.add(tag));
    } catch (error) {
      errors.push(
        `Failed to import note ${exportNote.id}: ${error instanceof Error ? error.message : 'Unknown error'}`
      );
    }

    // Report progress after each note
    if (onProgress) {
      onProgress(i + 1, totalNotes);
    }
  }

  return { imported, skipped, errors, attachments, tags };
}

/**
 * Download export data as JSON file
 *
 * Uses chunked JSON generation to avoid string length limits on large exports.
 */
export async function downloadExport(data: ExportData, filename?: string): Promise<void> {
  // Build JSON in chunks to avoid string length limits
  const chunks: string[] = [];

  // Opening structure
  chunks.push('{\n  "version": ');
  chunks.push(JSON.stringify(data.version));
  chunks.push(',\n  "exportDate": ');
  chunks.push(JSON.stringify(data.exportDate));
  chunks.push(',\n  "notes": [\n');

  // Add each note individually
  for (let i = 0; i < data.notes.length; i++) {
    if (i > 0) chunks.push(',\n');
    // Stringify each note with indentation
    const noteJson = JSON.stringify(data.notes[i], null, 2);
    // Indent the note JSON by 4 spaces
    chunks.push('    ' + noteJson.split('\n').join('\n    '));
  }

  chunks.push('\n  ]');

  // Add settings if present
  if (data.settings) {
    chunks.push(',\n  "settings": ');
    chunks.push(JSON.stringify(data.settings, null, 2).split('\n').join('\n  '));
  }

  chunks.push('\n}');

  const blob = new Blob(chunks, { type: 'application/json' });
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
