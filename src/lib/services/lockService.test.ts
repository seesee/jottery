import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { get } from 'svelte/store';

vi.mock('./initService', () => ({ lock: vi.fn() }));
vi.mock('./syncRepository', () => ({ syncRepository: { getMetadata: vi.fn() } }));
vi.mock('./syncService', () => ({ syncService: { syncNow: vi.fn() } }));

import { FINAL_SYNC_LIMIT_MS, lockApplication } from './lockService';
import { lock } from './initService';
import { syncRepository } from './syncRepository';
import { syncService } from './syncService';
import { isLocked, isLocking, settings } from '../stores/appStore';
import { registerPendingSave } from '../utils/pendingSave';
import { DEFAULT_SETTINGS } from '../types';

describe('lockApplication', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.mocked(lock).mockClear();
    vi.mocked(syncService.syncNow).mockReset();
    vi.mocked(syncRepository.getMetadata).mockResolvedValue({ apiKey: 'x' } as never);
    settings.set({ ...DEFAULT_SETTINGS, syncEnabled: true });
    isLocking.set(false);
    isLocked.set(false);
  });

  afterEach(() => {
    vi.useRealTimers();
    settings.set(DEFAULT_SETTINGS);
    isLocking.set(false);
  });

  it('blanks the UI before anything else', async () => {
    let blankedFirst = false;
    const stop = registerPendingSave(async () => { blankedFirst = get(isLocking); });
    vi.mocked(syncService.syncNow).mockResolvedValue({ success: true });

    await lockApplication();
    stop();

    expect(blankedFirst).toBe(true);
  });

  it('saves what the editor is holding, then syncs, then locks', async () => {
    const order: string[] = [];
    const stop = registerPendingSave(async () => { order.push('save'); });
    vi.mocked(syncService.syncNow).mockImplementation(async () => { order.push('sync'); return { success: true }; });
    vi.mocked(lock).mockImplementation(() => { order.push('lock'); });

    await lockApplication();
    stop();

    expect(order).toEqual(['save', 'sync', 'lock']);
    expect(get(isLocked)).toBe(true);
    expect(get(isLocking)).toBe(false);
  });

  it('locks when the last sync takes too long', async () => {
    vi.mocked(syncService.syncNow).mockReturnValue(new Promise(() => undefined));

    const locking = lockApplication();
    await vi.advanceTimersByTimeAsync(FINAL_SYNC_LIMIT_MS);
    await locking;

    expect(lock).toHaveBeenCalled();
    expect(get(isLocked)).toBe(true);
  });

  it('locks even if saving or syncing fails', async () => {
    const stop = registerPendingSave(async () => { throw new Error('no'); });

    await lockApplication();
    stop();

    expect(lock).toHaveBeenCalled();
  });

  it('does not start twice', async () => {
    vi.mocked(syncService.syncNow).mockResolvedValue({ success: true });

    await Promise.all([lockApplication(), lockApplication()]);

    expect(lock).toHaveBeenCalledTimes(1);
  });
});
