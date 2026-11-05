/**
 * Validation at the sync boundary.
 *
 * The server holds only ciphertext, and the design says it must never be able to
 * read notes, so everything it sends is treated as hostile: ids, timestamps, mime
 * types and flags are plaintext it can set without the key. Each record is checked
 * as it arrives, and one that fails is dropped so the rest of the sync carries on.
 *
 * What a failure does depends on the field:
 * - Anything structural (ids, dates, ciphertext) rejects the whole record. There
 *   is no safe way to store it, and a wrong date alone stops the note list loading.
 * - Display fields (language, colour, mime type, size) are reset, so that an
 *   unusual value from a newer client can't make a note disappear.
 *
 * Output is escaped separately, in the renderers. Each layer assumes the other
 * has a bug.
 */

import type {
  AttachmentRef,
  InboxItem,
  SyncNote,
  SyncNoteVersion,
  SyncPullResponse,
  SyncPushResponse,
  SyncRejected,
  SyncSavedSearch,
} from '../types';

export type Validated<T> = { ok: true; value: T } | { ok: false; reason: string };

const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

// type/subtype with optional parameters, as in RFC 6838
const MIME_PATTERN = /^[a-z0-9][a-z0-9!#$&^_.+-]*\/[a-z0-9][a-z0-9!#$&^_.+-]*(\s*;\s*[a-z0-9_.-]+=[a-z0-9_."-]+)*$/i;
const FALLBACK_MIME_TYPE = 'application/octet-stream';

// A pattern, not a closed list: a language this client doesn't know must survive,
// or the next edit here would blank it on every other device.
const LANGUAGE_PATTERN = /^[a-z0-9][a-z0-9+#._-]{0,39}$/i;
const COLOUR_PATTERN = /^[a-z0-9][a-z0-9_-]{0,39}$/i;
// Every client writes content hashes as standard base64 of a SHA-256 digest
const HASH_PATTERN = /^[A-Za-z0-9+/]{16,128}={0,2}$/;
// The protocol says a short ASCII string. Anything else isn't a cursor
const CURSOR_PATTERN = /^[\x21-\x7e]{0,256}$/;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isVersionNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0;
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every(item => typeof item === 'string');
}

export function isUuid(value: unknown): value is string {
  return typeof value === 'string' && UUID_PATTERN.test(value);
}

export function isTimestamp(value: unknown): value is string {
  return typeof value === 'string' && Number.isFinite(Date.parse(value));
}

function optionalTimestamp(value: unknown): string | undefined {
  return isTimestamp(value) ? value : undefined;
}

export function safeLanguage(value: unknown): string | undefined {
  // `in` also catches inherited names, which are unsafe as lookup keys
  return typeof value === 'string' && LANGUAGE_PATTERN.test(value) && !(value in {}) ? value : undefined;
}

export function safeColour(value: unknown): string | undefined {
  return typeof value === 'string' && COLOUR_PATTERN.test(value) && !(value in {}) ? value : undefined;
}

function safeHash(value: unknown): string | undefined {
  return typeof value === 'string' && HASH_PATTERN.test(value) ? value : undefined;
}

/** Null is meaningful here: it marks the first version of a note. */
function safeParentHash(value: unknown): string | null | undefined {
  return value === null ? null : safeHash(value);
}

function safeHashChain(value: unknown): string[] | undefined {
  if (!Array.isArray(value)) return undefined;
  return value.filter((hash): hash is string => safeHash(hash) !== undefined);
}

export function safeMimeType(value: unknown): string {
  return typeof value === 'string' && MIME_PATTERN.test(value) ? value : FALLBACK_MIME_TYPE;
}

/** Returns a clean copy of an attachment reference, or null if it can't be used. */
export function validateAttachmentRef(value: unknown): AttachmentRef | null {
  if (!isRecord(value) || !isUuid(value.id) || !isUuid(value.data) || typeof value.filename !== 'string') {
    return null;
  }

  const { id, data, filename, mimeType, size } = value;
  return {
    id,
    data,
    filename,
    mimeType: safeMimeType(mimeType),
    size: typeof size === 'number' && Number.isFinite(size) && size >= 0 ? size : 0,
  };
}

/** Returns clean copies of every reference, or null if any one of them is bad. */
function validateAttachmentRefs(value: unknown): AttachmentRef[] | null {
  if (!Array.isArray(value)) return null;
  const refs = value.map(validateAttachmentRef);
  return refs.every((ref): ref is AttachmentRef => ref !== null) ? refs : null;
}

/** Reasons are logged and shown in vault health, so they never quote the value. */
function fail<T>(reason: string): Validated<T> {
  return { ok: false, reason };
}

/** A list a response may leave out. Anything else in its place means the response is broken. */
function optionalList<T>(value: unknown, field: string): T[] {
  if (value === undefined || value === null) return [];
  if (!Array.isArray(value)) throw new Error(`Malformed sync response: ${field} is not a list`);
  return value as T[];
}

/**
 * Check the shape of a pull response before anything in it is applied. Records are
 * validated one at a time later, and a bad one is dropped. A page that is broken as
 * a whole is refused instead: accepting it would move the cursor past whatever it
 * should have held. Refusing costs nothing, because the same page comes again.
 */
export function validatePullResponse(value: unknown): SyncPullResponse {
  if (!isRecord(value)) throw new Error('Malformed sync response');
  if (!Array.isArray(value.notes)) throw new Error('Malformed sync response: notes is not a list');

  return {
    notes: value.notes as SyncNote[],
    deletions: optionalList(value.deletions, 'deletions'),
    attachments: optionalList(value.attachments, 'attachments'),
    versions: optionalList(value.versions, 'versions'),
    savedSearches: optionalList(value.savedSearches, 'savedSearches'),
    inboxItems: optionalList(value.inboxItems, 'inboxItems'),
    syncedAt: isTimestamp(value.syncedAt) ? value.syncedAt : undefined,
    totalCount: isVersionNumber(value.totalCount) ? value.totalCount : undefined,
    hasMore: value.hasMore === true,
    nextCursor: typeof value.nextCursor === 'string' && CURSOR_PATTERN.test(value.nextCursor) ? value.nextCursor : undefined,
  };
}

/** The same check for a push response. Absent lists are what an older server sends. */
export function validatePushResponse(value: unknown): SyncPushResponse {
  if (!isRecord(value)) throw new Error('Malformed sync response');

  return {
    accepted: optionalList(value.accepted, 'accepted'),
    rejected: optionalList(value.rejected, 'rejected'),
    attachmentWarnings: optionalList(value.attachmentWarnings, 'attachmentWarnings'),
    rejectedDeletions: optionalList<unknown>(value.rejectedDeletions, 'rejectedDeletions').filter(isUuid),
    skippedDeleted: optionalList<unknown>(value.skippedDeleted, 'skippedDeleted').filter(isUuid),
    errors: optionalList<unknown>(value.errors, 'errors').filter(item => typeof item === 'string'),
  };
}

export function validateRemoteNote(value: unknown): Validated<SyncNote> {
  if (!isRecord(value)) return fail('not a note');
  if (!isUuid(value.id)) return fail('invalid id');
  if (!isTimestamp(value.createdAt)) return fail('invalid createdAt');
  if (!isTimestamp(value.modifiedAt)) return fail('invalid modifiedAt');
  if (typeof value.content !== 'string') return fail('invalid content');
  if (!isStringArray(value.tags)) return fail('invalid tags');
  if (!isVersionNumber(value.version)) return fail('invalid version');

  const attachments = validateAttachmentRefs(value.attachments);
  if (!attachments) return fail('invalid attachment');

  return {
    ok: true,
    value: {
      id: value.id,
      createdAt: value.createdAt,
      modifiedAt: value.modifiedAt,
      content: value.content,
      tags: value.tags,
      attachments,
      pinned: value.pinned === true,
      archived: value.archived === true,
      archivedAt: optionalTimestamp(value.archivedAt),
      locked: value.locked === true,
      lockedAt: optionalTimestamp(value.lockedAt),
      deleted: value.deleted === true,
      deletedAt: optionalTimestamp(value.deletedAt),
      version: value.version,
      wordWrap: typeof value.wordWrap === 'boolean' ? value.wordWrap : undefined,
      syntaxLanguage: safeLanguage(value.syntaxLanguage),
      showPreview: typeof value.showPreview === 'boolean' ? value.showPreview : undefined,
      color: safeColour(value.color),
      contentHash: safeHash(value.contentHash),
      parentHash: safeParentHash(value.parentHash),
      hashChain: safeHashChain(value.hashChain),
    },
  };
}

export function validateRemoteVersion(value: unknown): Validated<SyncNoteVersion> {
  if (!isRecord(value)) return fail('not a version');
  if (!isUuid(value.noteId)) return fail('invalid noteId');
  if (!isVersionNumber(value.version)) return fail('invalid version');
  if (!isTimestamp(value.createdAt)) return fail('invalid createdAt');
  if (typeof value.content !== 'string') return fail('invalid content');
  if (!isStringArray(value.tags)) return fail('invalid tags');

  const attachments = validateAttachmentRefs(value.attachments);
  if (!attachments) return fail('invalid attachment');

  return {
    ok: true,
    value: {
      ...(value as unknown as SyncNoteVersion),
      syncedAt: optionalTimestamp(value.syncedAt) as string,
      attachments,
      syntaxLanguage: safeLanguage(value.syntaxLanguage),
      color: safeColour(value.color),
      contentHash: safeHash(value.contentHash),
      parentHash: safeParentHash(value.parentHash),
    },
  };
}

export function validateConflict(value: unknown): Validated<SyncRejected> {
  if (!isRecord(value)) return fail('not a conflict');
  if (!isUuid(value.id)) return fail('invalid id');
  if (!isTimestamp(value.serverModifiedAt)) return fail('invalid serverModifiedAt');
  if (typeof value.serverContent !== 'string') return fail('invalid serverContent');
  if (!isStringArray(value.serverTags)) return fail('invalid serverTags');
  if (!isVersionNumber(value.serverVersion)) return fail('invalid serverVersion');

  const serverAttachments = validateAttachmentRefs(value.serverAttachments ?? []);
  if (!serverAttachments) return fail('invalid attachment');

  return {
    ok: true,
    value: {
      ...(value as unknown as SyncRejected),
      serverAttachments,
      serverPinned: value.serverPinned === true,
      serverSyntaxLanguage: safeLanguage(value.serverSyntaxLanguage),
      serverWordWrap: typeof value.serverWordWrap === 'boolean' ? value.serverWordWrap : undefined,
      serverContentHash: safeHash(value.serverContentHash),
      serverParentHash: safeParentHash(value.serverParentHash),
      serverHashChain: safeHashChain(value.serverHashChain),
      ancestorHash: safeHash(value.ancestorHash),
      ancestorContent: typeof value.ancestorContent === 'string' ? value.ancestorContent : undefined,
      ancestorTags: isStringArray(value.ancestorTags) ? value.ancestorTags : undefined,
    },
  };
}

export function validateSavedSearch(value: unknown): Validated<SyncSavedSearch> {
  if (!isRecord(value)) return fail('not a saved search');
  if (!isUuid(value.id)) return fail('invalid id');
  if (typeof value.name !== 'string') return fail('invalid name');
  if (typeof value.query !== 'string') return fail('invalid query');
  if (!isTimestamp(value.createdAt)) return fail('invalid createdAt');
  if (!isTimestamp(value.modifiedAt)) return fail('invalid modifiedAt');

  return {
    ok: true,
    value: {
      ...(value as unknown as SyncSavedSearch),
      order: typeof value.order === 'number' && Number.isFinite(value.order) ? value.order : 0,
      deleted: value.deleted === true,
      deletedAt: optionalTimestamp(value.deletedAt),
    },
  };
}

/** Inbox content is plaintext by design, so only its shape can be checked. */
export function validateInboxItem(value: unknown): InboxItem | null {
  if (!isRecord(value) || !isUuid(value.id) || typeof value.content !== 'string'
    || !isTimestamp(value.createdAt) || !Array.isArray(value.tags)) {
    return null;
  }

  return {
    id: value.id,
    content: value.content,
    tags: value.tags.filter((tag): tag is string => typeof tag === 'string' && tag.length > 0),
    createdAt: value.createdAt,
    ...(typeof value.source === 'string' ? { source: value.source } : {}),
    sizeBytes: typeof value.sizeBytes === 'number' && Number.isFinite(value.sizeBytes) && value.sizeBytes >= 0 ? value.sizeBytes : 0,
  };
}
