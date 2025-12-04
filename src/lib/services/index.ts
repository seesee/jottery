/**
 * Barrel export for services
 */

// Crypto services
export { cryptoService, encryptJSON, decryptJSON, encryptStringArray, decryptStringArray } from './crypto';
export { keyManager, setupActivityListeners, removeActivityListeners } from './keyManager';

// Database
export { initDB, getDB, closeDB, deleteDB, STORES } from './db';

// Repositories
export { noteRepository } from './noteRepository';
export { attachmentRepository } from './attachmentRepository';
export { settingsRepository } from './settingsRepository';
export { encryptionRepository } from './encryptionRepository';

// Business logic services
export { noteService } from './noteService';
export { attachmentService } from './attachmentService';
export { isInitialized, initialize, unlock, lock, isLocked, changePassword } from './initService';
export { exportAllNotes, exportNotes, importNotes, downloadExport, parseImportFile } from './exportService';
export { searchService, indexNotes, parseSearchQuery, searchNotes, getSearchSuggestions } from './searchService';
export { tagService, getAllTags, getTagStats, getPopularTags, getNotesByTag, getNotesByTags, normalizeTag, isValidTag, parseTagString } from './tagService';
export { initI18n, getInitialLocale, AVAILABLE_LOCALES, DEFAULT_LOCALE } from './i18nService';
