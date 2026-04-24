/**
 * Auto-lock service to lock application after inactivity
 *
 * Enforces two timeouts:
 * - autoLockTimeout (idle): set by settings.autoLockTimeout — resets on activity
 * - persistSessionTimeout (absolute): an upper bound on the sessionStorage-based
 *   "Persist Session Across Page Refresh" cache. It does NOT reset on activity,
 *   so an active user can otherwise type past its expiry with a master key that
 *   has silently become unsynced / no longer eligible for auto-unlock on reload.
 *   We re-check session validity on every interaction and visibility change so
 *   the UI locks the moment the window has closed.
 */

import { isLocked } from '../stores/appStore';
import { lock } from './initService';
import { settingsRepository } from './settingsRepository';
import { hasValidSession } from './sessionStorageService';
import { hasStoredPassword } from './passwordStorageService';

let lastActivityTime: number = Date.now();
let autoLockTimeout: number = 15 * 60 * 1000; // 15 minutes in milliseconds
let checkInterval: number | null = null;

// Cached at startAutoLock() so every synchronous activity handler can decide
// without an async settings read.
let persistSessionEnabled = false;
let persistSessionTimeoutMinutes = 30;

const ACTIVITY_EVENTS = ['mousedown', 'mousemove', 'keypress', 'scroll', 'touchstart', 'click'] as const;

/**
 * Whether the absolute persist-session window has elapsed.
 * Returns false when the feature is off — in that mode the sessionStorage
 * password isn't relevant, so there's nothing to expire.
 */
function isPersistSessionExpired(): boolean {
  if (!persistSessionEnabled) return false;
  return !hasValidSession(persistSessionTimeoutMinutes);
}

/**
 * Activity handler: either extend the idle window or lock immediately if
 * the persist-session window has elapsed.
 */
function onActivity() {
  if (isPersistSessionExpired()) {
    void performLock();
    return;
  }
  lastActivityTime = Date.now();
}

/**
 * Visibility handler: when the tab regains focus, re-validate before letting
 * the user keep working.
 */
function onVisibilityChange() {
  if (document.visibilityState !== 'visible') return;
  if (isPersistSessionExpired()) {
    void performLock();
  }
}

/**
 * Periodic tick: backstop in case no activity or visibility event arrives.
 */
async function checkAutoLock() {
  const now = Date.now();
  const timeSinceActivity = now - lastActivityTime;

  if (timeSinceActivity >= autoLockTimeout || isPersistSessionExpired()) {
    await performLock();
  }
}

/**
 * Lock the application
 * Skips lock if there's a stored password or valid persist session that would auto-unlock
 */
async function performLock() {
  try {
    // Check if remember password is enabled - would auto-unlock immediately
    const hasPassword = await hasStoredPassword();
    if (hasPassword) {
      // Reset activity timer instead of locking - no point locking if we'd unlock immediately
      lastActivityTime = Date.now();
      return;
    }

    // Check if persist session is enabled with a valid session - would auto-unlock immediately
    const settings = await settingsRepository.get();
    if (settings.persistSession) {
      const sessionTimeout = settings.persistSessionTimeout ?? 30;
      if (hasValidSession(sessionTimeout)) {
        // Reset activity timer instead of locking - no point locking if we'd unlock immediately
        lastActivityTime = Date.now();
        return;
      }
    }
  } catch (error) {
    // If we can't check settings, proceed with lock for safety
    console.warn('[AutoLock] Failed to check session state, proceeding with lock:', error);
  }

  lock();
  isLocked.set(true);
  stopAutoLock();
}

/**
 * Start auto-lock monitoring
 */
export function startAutoLock(timeoutMinutes: number = 15): void {
  // Stop existing interval if any
  stopAutoLock();

  // Set timeout in milliseconds
  autoLockTimeout = timeoutMinutes * 60 * 1000;
  lastActivityTime = Date.now();

  // Cache persist-session settings so the activity handler can run synchronously.
  // Any subsequent changes (toggle in settings, timeout bump) re-enter via
  // App.svelte's reactive block, which calls startAutoLock() again.
  settingsRepository
    .get()
    .then((settings) => {
      persistSessionEnabled = !!settings.persistSession;
      persistSessionTimeoutMinutes = settings.persistSessionTimeout ?? 30;
    })
    .catch((error) => {
      console.warn('[AutoLock] Failed to cache persist-session settings:', error);
      persistSessionEnabled = false;
    });

  // Set up activity listeners
  ACTIVITY_EVENTS.forEach((event) => {
    document.addEventListener(event, onActivity, { passive: true });
  });

  // Re-check when the tab comes back into focus — a window can elapse entirely
  // while the tab is backgrounded and no activity event ever fires.
  document.addEventListener('visibilitychange', onVisibilityChange);

  // Check every minute
  checkInterval = window.setInterval(checkAutoLock, 60 * 1000);
}

/**
 * Stop auto-lock monitoring
 */
export function stopAutoLock(): void {
  if (checkInterval !== null) {
    clearInterval(checkInterval);
    checkInterval = null;
  }

  // Remove activity listeners (must use same options as when added)
  ACTIVITY_EVENTS.forEach((event) => {
    document.removeEventListener(event, onActivity, { passive: true } as EventListenerOptions);
  });
  document.removeEventListener('visibilitychange', onVisibilityChange);
}

/**
 * Update auto-lock timeout
 */
export function updateAutoLockTimeout(timeoutMinutes: number): void {
  autoLockTimeout = timeoutMinutes * 60 * 1000;
}

/**
 * Auto-lock service interface
 */
export const autoLockService = {
  start: startAutoLock,
  stop: stopAutoLock,
  updateTimeout: updateAutoLockTimeout,
};
