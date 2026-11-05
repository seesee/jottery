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
import { getCurrentNotebook } from '../utils/notebookPath';
import { flushPendingSave } from '../utils/pendingSave';

export const FINAL_SYNC_LIMIT_MS = 10_000;

const LOCK_MESSAGE = 'lock';

/**
 * Tabs of the same notebook, which share a vault. Only a lock someone asked for goes
 * here. Auto-lock doesn't: a tab left idle shouldn't lock the one being used.
 */
function lockChannel(): BroadcastChannel | null {
  return typeof BroadcastChannel === 'undefined'
    ? null
    : new BroadcastChannel(`jottery-lock:${getCurrentNotebook().id}`);
}

/** Lock this tab when another locks. Returns a function that stops listening. */
export function listenForLockElsewhere(): () => void {
  const channel = lockChannel();
  // This tab hears its own lock too, which is harmless: isLocking turns it away
  const onMessage = () => {
    if (!get(isLocked)) {
      void lockApplication({ announce: false });
    }
  };

  channel?.addEventListener('message', onMessage);
  return () => {
    channel?.removeEventListener('message', onMessage);
    channel?.close?.();
  };
}

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

export async function lockApplication({ announce = true } = {}): Promise<void> {
  if (get(isLocking)) {
    return;
  }
  isLocking.set(true);

  if (announce) {
    const channel = lockChannel();
    channel?.postMessage(LOCK_MESSAGE);
    channel?.close?.();
  }

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
