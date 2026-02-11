/**
 * Auto-lock service to lock application after inactivity
 */

import { isLocked } from '../stores/appStore';
import { lock } from './initService';
import { settingsRepository } from './settingsRepository';
import { hasValidSession } from './sessionStorageService';
import { hasStoredPassword } from './passwordStorageService';

let lastActivityTime: number = Date.now();
let autoLockTimeout: number = 15 * 60 * 1000; // 15 minutes in milliseconds
let checkInterval: number | null = null;

/**
 * Update last activity timestamp
 */
function updateActivity() {
  lastActivityTime = Date.now();
}

/**
 * Check if should auto-lock
 */
async function checkAutoLock() {
  const now = Date.now();
  const timeSinceActivity = now - lastActivityTime;

  if (timeSinceActivity >= autoLockTimeout) {
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

  // Set up activity listeners
  const events = ['mousedown', 'mousemove', 'keypress', 'scroll', 'touchstart', 'click'];
  events.forEach(event => {
    document.addEventListener(event, updateActivity, { passive: true });
  });

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
  const events = ['mousedown', 'mousemove', 'keypress', 'scroll', 'touchstart', 'click'];
  events.forEach(event => {
    document.removeEventListener(event, updateActivity, { passive: true } as EventListenerOptions);
  });
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
