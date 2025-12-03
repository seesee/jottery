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
export { isInitialized, initialize, unlock, lock, isLocked, changePassword } from './initService';
export { exportAllNotes, exportNotes, importNotes, downloadExport, parseImportFile } from './exportService';
