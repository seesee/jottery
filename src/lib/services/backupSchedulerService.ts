/**
 * Backup Scheduler Service
 *
 * Manages backup reminders and automated backups:
 * - Periodic reminders prompting users to create backups
 * - Automatic backup downloads based on time or change count
 *
 * Lifecycle:
 * - start() is called on app unlock
 * - stop() is called on app lock
 * - onNoteUpdated() is called when notes are created/modified
 */

import { writable, get } from 'svelte/store';
import { settingsRepository } from './settingsRepository';
import { createBackup, downloadBackup } from './backupService';
import { toast } from '../utils/toast.svelte';
import {
  BACKUP_REMINDER_CHECK_INTERVAL_MS,
  BACKUP_AUTO_CHECK_INTERVAL_MS,
  MS_PER_DAY,
  MS_PER_WEEK,
  MS_PER_MONTH,
} from '../constants';

/**
 * Store: Whether to show the backup reminder banner
 */
export const showBackupReminder = writable<boolean>(false);

/**
 * Store: Last backup timestamp (for display)
 */
export const lastBackupDate = writable<string | null>(null);

/**
 * Store: Whether an auto-backup is in progress
 */
export const isAutoBackingUp = writable<boolean>(false);

// Timer handles
let reminderTimer: ReturnType<typeof setInterval> | null = null;
let autoBackupTimer: ReturnType<typeof setInterval> | null = null;

// Track if we've already dismissed the reminder this session
let reminderDismissedThisSession = false;

/**
 * Get the interval in milliseconds for a given frequency
 */
function getFrequencyMs(frequency: 'daily' | 'weekly' | 'monthly'): number {
  switch (frequency) {
    case 'daily':
      return MS_PER_DAY;
    case 'weekly':
      return MS_PER_WEEK;
    case 'monthly':
      return MS_PER_MONTH;
    default:
      return MS_PER_WEEK;
  }
}

/**
 * Check if a backup reminder should be shown
 */
async function checkBackupReminder(): Promise<void> {
  // Skip if already dismissed this session
  if (reminderDismissedThisSession) {
    return;
  }

  try {
    const settings = await settingsRepository.get();

    // Skip if reminders are disabled
    if (!settings.backupReminderEnabled) {
      showBackupReminder.set(false);
      return;
    }

    const lastBackup = settings.lastBackupAt;
    lastBackupDate.set(lastBackup || null);

    // If never backed up, show reminder immediately
    if (!lastBackup) {
      showBackupReminder.set(true);
      return;
    }

    // Check if enough time has passed since last backup
    const frequency = settings.backupReminderFrequency || 'weekly';
    const intervalMs = getFrequencyMs(frequency);
    const lastBackupTime = new Date(lastBackup).getTime();
    const now = Date.now();

    if (now - lastBackupTime >= intervalMs) {
      showBackupReminder.set(true);
    } else {
      showBackupReminder.set(false);
    }
  } catch (error) {
    console.error('[BackupScheduler] Error checking backup reminder:', error);
  }
}

/**
 * Check if an automatic backup should be triggered
 */
async function checkAutoBackup(): Promise<void> {
  try {
    const settings = await settingsRepository.get();

    // Skip if auto-backup is disabled
    if (!settings.autoBackupEnabled) {
      return;
    }

    const frequency = settings.autoBackupFrequency || 'weekly';
    const lastBackup = settings.lastBackupAt;

    // Check based on frequency type
    if (frequency === 'on-changes') {
      // Trigger based on change count
      const threshold = settings.autoBackupAfterChanges || 10;
      const changes = settings.noteUpdatesSinceBackup || 0;

      if (changes >= threshold) {
        await triggerAutoBackup();
      }
    } else {
      // Trigger based on time
      if (!lastBackup) {
        // Never backed up - trigger now
        await triggerAutoBackup();
        return;
      }

      const intervalMs = getFrequencyMs(frequency);
      const lastBackupTime = new Date(lastBackup).getTime();
      const now = Date.now();

      if (now - lastBackupTime >= intervalMs) {
        await triggerAutoBackup();
      }
    }
  } catch (error) {
    console.error('[BackupScheduler] Error checking auto-backup:', error);
  }
}

/**
 * Trigger an automatic backup download
 */
async function triggerAutoBackup(): Promise<void> {
  // Prevent concurrent backups
  if (get(isAutoBackingUp)) {
    return;
  }

  isAutoBackingUp.set(true);

  try {
    console.log('[BackupScheduler] Triggering automatic backup...');

    const backup = await createBackup();
    downloadBackup(backup);

    // Record the backup
    await recordBackup();

    console.log('[BackupScheduler] Automatic backup completed');
  } catch (error) {
    console.error('[BackupScheduler] Auto-backup failed:', error);
    // Show error toast - backup downloads can fail silently if browser blocks
    toast.error('Automatic backup failed. Please create a manual backup.');
  } finally {
    isAutoBackingUp.set(false);
  }
}

/**
 * Start the backup scheduler timers
 * Called when the app is unlocked
 */
export async function start(): Promise<void> {
  console.log('[BackupScheduler] Starting...');

  // Reset session state
  reminderDismissedThisSession = false;

  // Initial checks
  await checkBackupReminder();
  await checkAutoBackup();

  // Set up periodic checks
  reminderTimer = setInterval(checkBackupReminder, BACKUP_REMINDER_CHECK_INTERVAL_MS);
  autoBackupTimer = setInterval(checkAutoBackup, BACKUP_AUTO_CHECK_INTERVAL_MS);
}

/**
 * Stop the backup scheduler timers
 * Called when the app is locked
 */
export function stop(): void {
  console.log('[BackupScheduler] Stopping...');

  if (reminderTimer) {
    clearInterval(reminderTimer);
    reminderTimer = null;
  }

  if (autoBackupTimer) {
    clearInterval(autoBackupTimer);
    autoBackupTimer = null;
  }

  // Hide the reminder banner
  showBackupReminder.set(false);
}

/**
 * Called when a note is updated (created, modified, deleted)
 * Increments the change counter for on-changes auto-backup
 */
export async function onNoteUpdated(): Promise<void> {
  try {
    const settings = await settingsRepository.get();
    const currentCount = settings.noteUpdatesSinceBackup || 0;

    await settingsRepository.update({
      noteUpdatesSinceBackup: currentCount + 1,
    });

    // Check if we should trigger an auto-backup now
    if (settings.autoBackupEnabled && settings.autoBackupFrequency === 'on-changes') {
      const threshold = settings.autoBackupAfterChanges || 10;
      if (currentCount + 1 >= threshold) {
        await triggerAutoBackup();
      }
    }
  } catch (error) {
    console.error('[BackupScheduler] Error updating note counter:', error);
  }
}

/**
 * Record that a backup was created
 * Resets the change counter and updates lastBackupAt
 */
export async function recordBackup(): Promise<void> {
  try {
    const now = new Date().toISOString();

    await settingsRepository.update({
      lastBackupAt: now,
      noteUpdatesSinceBackup: 0,
    });

    // Update stores
    lastBackupDate.set(now);
    showBackupReminder.set(false);
    reminderDismissedThisSession = false;

    console.log('[BackupScheduler] Backup recorded at:', now);
  } catch (error) {
    console.error('[BackupScheduler] Error recording backup:', error);
  }
}

/**
 * Dismiss the backup reminder banner temporarily (until next check or session)
 */
export function dismissReminder(): void {
  showBackupReminder.set(false);
  reminderDismissedThisSession = true;
}

/**
 * Backup scheduler service singleton
 */
export const backupSchedulerService = {
  start,
  stop,
  onNoteUpdated,
  recordBackup,
  dismissReminder,
  // Expose stores for components
  showBackupReminder,
  lastBackupDate,
  isAutoBackingUp,
};
