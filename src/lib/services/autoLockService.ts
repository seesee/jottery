/**
 * Auto-lock service to lock application after inactivity
 */

import { isLocked } from '../stores/appStore';
import { lock } from './initService';

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
function checkAutoLock() {
  const now = Date.now();
  const timeSinceActivity = now - lastActivityTime;

  if (timeSinceActivity >= autoLockTimeout) {
    performLock();
  }
}

/**
 * Lock the application
 */
function performLock() {
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

  // Remove activity listeners
  const events = ['mousedown', 'mousemove', 'keypress', 'scroll', 'touchstart', 'click'];
  events.forEach(event => {
    document.removeEventListener(event, updateActivity);
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
