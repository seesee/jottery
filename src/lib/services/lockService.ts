/**
 * Locking on request, as opposed to auto-lock. The vault has to disappear at once,
 * so the UI blanks first. Saving and a last sync are then worth waiting for, but only
 * so long: a backlog must not keep a vault open and unattended for minutes.
 */
import { get } from 'svelte/store';
import { lock } from './initService';
import { syncRepository } from './syncRepository';
import { syncService } from './syncService';
import { isLocked, isLocking, settings } from '../stores/appStore';
import { flushPendingSave } from '../utils/pendingSave';

export const FINAL_SYNC_LIMIT_MS = 10_000;

async function finalSync(): Promise<void> {
  if (!get(settings).syncEnabled || !(await syncRepository.getMetadata())?.apiKey) {
    return;
  }

  let timer: ReturnType<typeof setTimeout> | undefined;
  const limit = new Promise<void>(resolve => {
    timer = setTimeout(resolve, FINAL_SYNC_LIMIT_MS);
  });
  await Promise.race([syncService.syncNow(), limit]);
  clearTimeout(timer);
}

export async function lockApplication(): Promise<void> {
  if (get(isLocking)) {
    return;
  }
  isLocking.set(true);

  try {
    await flushPendingSave();
    await finalSync();
  } catch (error) {
    // The lock matters more than either of these
    console.error('[Lock] Error during pre-lock save or sync:', error);
  }

  lock();
  isLocked.set(true);
  isLocking.set(false);
}
