/**
 * Barrel export for services
 */

// Crypto services
export { cryptoService, encryptJSON, decryptJSON, encryptStringArray, decryptStringArray } from './crypto';
export { keyManager, setupActivityListeners, removeActivityListeners, requireMasterKey, ApplicationLockedError } from './keyManager';
export { passwordStorageService } from './passwordStorageService';
export { sessionStorageService } from './sessionStorageService';

// Database
export { initDB, getDB, closeDB, deleteDB, STORES } from './db';

// Repositories
export { noteRepository } from './noteRepository';
export { attachmentRepository } from './attachmentRepository';
export { settingsRepository } from './settingsRepository';
export { encryptionRepository } from './encryptionRepository';
export { syncRepository } from './syncRepository';
export { versionRepository } from './versionRepository';

// Business logic services
export { syncService } from './syncService';
export { noteService } from './noteService';
export { createSyncRecoveryNote, deleteSyncRecoveryNote } from './syncRecoveryService';
export { attachmentService } from './attachmentService';
export { conflictService } from './conflictService';
export { isInitialized, initialize, unlock, lock, isLocked, changePassword, restoreFromBackup } from './initService';
export { exportAllNotes, exportNotes, importNotes, downloadExport, parseImportFile } from './exportService';
export { searchService, indexNotes, parseSearchQuery, searchNotes, getSearchSuggestions } from './searchService';
export { tagService, getAllTags, getTagStats, getPopularTags, getNotesByTag, getNotesByTags, normalizeTag, isValidTag, parseTagString } from './tagService';
export { initI18n, getInitialLocale, AVAILABLE_LOCALES, DEFAULT_LOCALE } from './i18nService';
export { appUpdateService, updateAvailable, newVersionInfo } from './appUpdateService';
export { backupService, createBackup, validateBackup, verifyBackupPassword, restoreBackup, downloadBackup, getBackupStats } from './backupService';
export type { BackupData, BackupValidationResult, RestoreProgressCallback } from './backupService';
export { backupSchedulerService, showBackupReminder, lastBackupDate, isAutoBackingUp } from './backupSchedulerService';
