/**
 * Export/Import service for notes data
 */

import type { EncryptionResult, ExportData, ExportNote, ExportAttachment, Note, Attachment } from '../types';
import { noteRepository } from './noteRepository';
import { attachmentRepository } from './attachmentRepository';
import { noteService } from './noteService';
import { keyManager } from './keyManager';
import { cryptoService, decryptStringArray } from './crypto';
import { arrayBufferToBase64, base64ToArrayBuffer } from '../utils/base64';
import { isUuid, safeColour, safeLanguage, safeMimeType } from './syncValidation';
import { toIsoTimestamp } from '../utils/timestamps';

const EXPORT_VERSION = '1.0';

/** A note from an import file, after checking. It has no id if it needs a new one. */
type ImportedNote = Omit<ExportNote, 'id'> & { id?: string };

/**
 * Check a note from an import file. The file can come from another app or another
 * person, so nothing in it is trusted. A bad field is replaced where there's a safe
 * default. Only something that isn't a note at all is refused.
 *
 * showPreview is always off, so an imported note can't render itself on selection.
 */
function sanitiseImportedNote(value: unknown): ImportedNote | null {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return null;

  const note = value as Record<string, unknown>;
  if (typeof note.content !== 'string') return null;

  const now = new Date().toISOString();
  const rawTags = typeof note.tags === 'string' ? [note.tags] : note.tags;

  return {
    id: isUuid(note.id) ? note.id : undefined,
    createdAt: toIsoTimestamp(note.createdAt) ?? now,
    modifiedAt: toIsoTimestamp(note.modifiedAt) ?? now,
    content: note.content,
    tags: Array.isArray(rawTags)
      ? rawTags.filter((tag): tag is string => typeof tag === 'string' && tag.trim().length > 0)
      : [],
    attachments: Array.isArray(note.attachments) ? (note.attachments as ExportAttachment[]) : [],
    pinned: note.pinned === true,
    archived: note.archived === true,
    locked: note.locked === true,
    wordWrap: typeof note.wordWrap === 'boolean' ? note.wordWrap : undefined,
    syntaxLanguage: safeLanguage(note.syntaxLanguage),
    showPreview: false,
    color: safeColour(note.color),
  };
}

/**
 * Progress callback for export operations
 */
export type ExportProgressCallback = (progress: {
  phase: 'counting' | 'exporting' | 'complete';
  current?: number;
  total?: number;
}) => void;

/**
 * Export all notes to JSON format
 */
export async function exportAllNotes(onProgress?: ExportProgressCallback): Promise<ExportData> {
  const masterKey = keyManager.getMasterKey();
  if (!masterKey) {
    throw new Error('Application is locked');
  }

  onProgress?.({ phase: 'counting' });

  // Everything but the recycle bin. Someone moving away must get their archive too.
  const notes = await noteRepository.getAllNonDeleted();
  const total = notes.length;
  const exportNotes: ExportNote[] = [];
  const skipped = { notes: 0, attachments: 0 };

  for (let i = 0; i < notes.length; i++) {
    try {
      const exportNote = await convertNoteToExport(notes[i], masterKey.key);
      skipped.attachments += notes[i].attachments.length - exportNote.attachments.length;
      exportNotes.push(exportNote);
    } catch (error) {
      console.error(`Failed to export note ${notes[i].id}:`, error);
      skipped.notes++;
    }
    onProgress?.({ phase: 'exporting', current: i + 1, total });
  }

  // Export color settings
  const { settingsRepository } = await import('./settingsRepository');
  const userSettings = await settingsRepository.get();

  onProgress?.({ phase: 'complete', current: total, total });

  return {
    version: EXPORT_VERSION,
    exportDate: new Date().toISOString(),
    notes: exportNotes,
    settings: {
      colorPalette: userSettings.colorPalette,
      tagColors: userSettings.tagColors,
    },
    skipped: skipped.notes + skipped.attachments > 0 ? skipped : undefined,
  };
}

/**
 * Export filtered notes (by IDs)
 */
export async function exportNotes(
  noteIds: string[],
  onProgress?: ExportProgressCallback
): Promise<ExportData> {
  const masterKey = keyManager.getMasterKey();
  if (!masterKey) {
    throw new Error('Application is locked');
  }

  const notes = await noteRepository.getByIds(noteIds);
  const exportNotes: ExportNote[] = [];
  const skipped = { notes: 0, attachments: 0 };

  for (const [index, note] of notes.entries()) {
    try {
      const exportNote = await convertNoteToExport(note, masterKey.key);
      skipped.attachments += note.attachments.length - exportNote.attachments.length;
      exportNotes.push(exportNote);
    } catch (error) {
      console.error(`Failed to export note ${note.id}:`, error);
      skipped.notes++;
    }
    onProgress?.({ phase: 'exporting', current: index + 1, total: notes.length });
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
    skipped: skipped.notes + skipped.attachments > 0 ? skipped : undefined,
  };
}

/**
 * The ciphertext in a stored field, or null for plain text from before notes were
 * encrypted. Only that case may be used as it stands: a value that is ciphertext and
 * won't decrypt has to fail, or the export would hold a blob of it as the note's text.
 */
function parseCiphertext(value: string): EncryptionResult | null {
  try {
    const parsed = JSON.parse(value);
    return typeof parsed?.ciphertext === 'string' && typeof parsed?.iv === 'string' ? parsed : null;
  } catch {
    return null;
  }
}

/**
 * Convert a note to export format (decrypt all fields). Throws if the note can't be
 * read, and the caller leaves it out and reports it.
 */
async function convertNoteToExport(note: Note, key: CryptoKey): Promise<ExportNote> {
  // Decrypt content
  const encryptedContent = parseCiphertext(note.content);
  const content = encryptedContent ? await cryptoService.decryptText(encryptedContent, key) : note.content;

  // Decrypt tags
  let tags: string[] = [];
  if (note.tags.length > 0 && note.tags[0]) {
    const encryptedTags = parseCiphertext(note.tags[0]);
    tags = encryptedTags ? await decryptStringArray(encryptedTags, key) : note.tags;
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
      const filenameEncrypted = parseCiphertext(attachment.filename);
      const filename = filenameEncrypted
        ? await cryptoService.decryptText(filenameEncrypted, key)
        : attachment.filename;

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
    const exportNote = sanitiseImportedNote(data.notes[i]);
    if (!exportNote) {
      errors.push(`Note ${i + 1} is not a valid note and was not imported`);
      onProgress?.(i + 1, totalNotes);
      continue;
    }

    try {
      // Check if note already exists by ID
      const existingNote = exportNote.id ? await noteRepository.getById(exportNote.id) : null;

      // Determine if we should preserve the original ID
      let useOriginalId = true;

      if (existingNote) {
        if (strategy === 'skip') {
          skipped++;
          continue;
        } else if (strategy === 'replace') {
          // Delete existing note, then recreate with same ID
          await noteRepository.delete(existingNote.id);
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
            if (typeof exportAttachment?.filename !== 'string' || typeof exportAttachment.data !== 'string') {
              throw new Error('Attachment has no filename or data');
            }

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
              mimeType: safeMimeType(exportAttachment.mimeType),
              size: fileData.byteLength,
              data: attachmentId, // Reference to blob storage
            });
          } catch (attachmentError) {
            console.error('Failed to import attachment:', attachmentError);
            errors.push(`An attachment on note ${i + 1} could not be imported`);
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
        archived: exportNote.archived,
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
        `Failed to import note ${i + 1}: ${error instanceof Error ? error.message : 'Unknown error'}`
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
