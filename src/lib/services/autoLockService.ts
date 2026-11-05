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

import { get } from 'svelte/store';
import { isLocked, settings as settingsStore } from '../stores/appStore';
import { lock } from './initService';
import { settingsRepository } from './settingsRepository';
import { hasStoredSession, hasValidSession } from './sessionStorageService';
import { hasStoredPassword } from './passwordStorageService';

let lastActivityTime: number = Date.now();
let autoLockTimeout: number = 15 * 60 * 1000; // 15 minutes in milliseconds
let checkInterval: number | null = null;

// Deliberate activity only: a mouse left on a desk can drift, and that's no reason to
// keep a vault open. Listened for on the way down, as editors stop some from bubbling.
const ACTIVITY_EVENTS = ['mousedown', 'keydown', 'scroll', 'touchstart'] as const;
const ACTIVITY_LISTENER = { passive: true, capture: true };

const CHECK_INTERVAL_MS = 15 * 1000;

/**
 * Whether the absolute persist-session window has elapsed.
 * Returns false when the feature is off — in that mode the sessionStorage
 * password isn't relevant, so there's nothing to expire.
 */
export function isPersistSessionExpired(): boolean {
  const { persistSession, persistSessionTimeout } = get(settingsStore);
  if (!persistSession) return false;
  // Nothing stored isn't the same as expired. If the store failed, treating it as
  // expired would lock on the first mouse move, before Settings could be reached.
  if (!hasStoredSession()) return false;
  return !hasValidSession(persistSessionTimeout ?? 30);
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
 * Lock the application, unless Remember Password would unlock it again at once.
 * Persist Session doesn't count: it survives a reload, not walking away, and locking
 * ends it.
 */
async function performLock() {
  try {
    // A stored password alone isn't enough: one left behind must not switch auto-lock off
    const settings = await settingsRepository.get();
    if (settings.rememberPassword && await hasStoredPassword()) {
      // Reset activity timer instead of locking - no point locking if we'd unlock immediately
      lastActivityTime = Date.now();
      return;
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

  // Set up activity listeners
  ACTIVITY_EVENTS.forEach((event) => {
    document.addEventListener(event, onActivity, ACTIVITY_LISTENER);
  });

  // Re-check when the tab comes back into focus — a window can elapse entirely
  // while the tab is backgrounded and no activity event ever fires.
  document.addEventListener('visibilitychange', onVisibilityChange);

  checkInterval = window.setInterval(checkAutoLock, CHECK_INTERVAL_MS);
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
    document.removeEventListener(event, onActivity, ACTIVITY_LISTENER);
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
