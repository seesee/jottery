/**
 * Application constants
 *
 * Centralised location for magic numbers and configuration values.
 * Grouped by domain for easy discovery and maintenance.
 */

// =============================================================================
// UI Constants - Virtual Scrolling
// =============================================================================

/** Initial estimated height per note item in pixels (before measurement) */
export const VIRTUAL_SCROLL_ESTIMATED_ITEM_HEIGHT = 80;

/** Number of items to render outside viewport for smooth scrolling */
export const VIRTUAL_SCROLL_OVERSCAN = 15;

/** Minimum items to render to prevent empty/broken scroll states */
export const VIRTUAL_SCROLL_MIN_VISIBLE_ITEMS = 30;

/** Batch size for processing notes in conflict checks */
export const CONFLICT_CHECK_BATCH_SIZE = 50;

// =============================================================================
// UI Constants - Editor
// =============================================================================

/** Delay in milliseconds before auto-saving after user stops typing */
export const EDITOR_AUTOSAVE_DELAY_MS = 1000;

/** Delay to clear drag state after drop completes (outliner) */
export const OUTLINER_DRAG_CLEAR_DELAY_MS = 100;

// =============================================================================
// UI Constants - Animations & Transitions
// =============================================================================

/** Duration for lock animation in milliseconds */
export const LOCK_ANIMATION_DURATION_MS = 1200;

/** Interval for landing page carousel auto-advance in milliseconds */
export const CAROUSEL_INTERVAL_MS = 10000;

/** Polling interval for unlock state check in milliseconds */
export const UNLOCK_POLL_INTERVAL_MS = 50;

// =============================================================================
// Search Constants
// =============================================================================

/** Maximum filtered results before hiding cross-mode match hint */
export const SEARCH_MAX_RESULTS_FOR_CROSS_MODE_HINT = 10;

// =============================================================================
// Sync Constants
// =============================================================================

/** Number of notes to push per batch to avoid memory limits */
export const SYNC_PUSH_BATCH_SIZE = 100;

/** Number of notes to pull per batch (pagination) */
export const SYNC_PULL_BATCH_SIZE = 100;

/** Default auto-sync interval in minutes */
export const SYNC_AUTO_INTERVAL_MINUTES = 5;

/** Minimum items to sync before showing percentage progress */
export const SYNC_PROGRESS_THRESHOLD = 5;

// =============================================================================
// Crypto Constants
// =============================================================================

/**
 * PBKDF2 iteration count for key derivation.
 * Higher values = more secure but slower unlock.
 * 100,000 is OWASP recommended minimum as of 2023.
 */
export const CRYPTO_PBKDF2_ITERATIONS = 100000;

// =============================================================================
// Security Constants
// =============================================================================

/** Default auto-lock timeout in minutes */
export const DEFAULT_AUTO_LOCK_TIMEOUT_MINUTES = 15;

// =============================================================================
// Limits & Thresholds
// =============================================================================

/** Maximum lines to process in calculator extension */
export const CALC_MAX_LINES = 500;

/** Maximum errors to display in calculator extension */
export const CALC_MAX_TOTAL_ERRORS = 50;

/** Maximum length for notebook ID in path */
export const MAX_NOTEBOOK_ID_LENGTH = 50;

/** Default batch size for note operations (e.g., getAllNotes) */
export const NOTE_OPERATION_BATCH_SIZE = 50;

// =============================================================================
// App Update Constants
// =============================================================================

/** Interval for checking app updates in milliseconds (5 minutes) */
export const APP_UPDATE_CHECK_INTERVAL_MS = 5 * 60 * 1000;
