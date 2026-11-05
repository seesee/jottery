import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { get } from 'svelte/store';

vi.mock('./initService', () => ({ lock: vi.fn() }));
vi.mock('./syncRepository', () => ({ syncRepository: { getMetadata: vi.fn() } }));
vi.mock('./syncService', () => ({ syncService: { syncNow: vi.fn() } }));

import { FINAL_SYNC_LIMIT_MS, listenForLockElsewhere, lockApplication } from './lockService';
import { lock } from './initService';
import { syncRepository } from './syncRepository';
import { syncService } from './syncService';
import { isLocked, isLocking, settings } from '../stores/appStore';
import { registerPendingSave } from '../utils/pendingSave';
import { DEFAULT_SETTINGS } from '../types';

describe('locking in another tab', () => {
  const posted: unknown[] = [];
  const listeners = new Set<() => void>();

  class FakeChannel {
    static names: string[] = [];
    constructor(name: string) { FakeChannel.names.push(name); }
    postMessage(message: unknown) { posted.push(message); }
    addEventListener(_: string, listener: () => void) { listeners.add(listener); }
    removeEventListener(_: string, listener: () => void) { listeners.delete(listener); }
  }

  const lockElsewhere = () => [...listeners].forEach(listener => listener());
  let stop: () => void;

  beforeEach(() => {
    posted.length = 0;
    listeners.clear();
    vi.stubGlobal('BroadcastChannel', FakeChannel);
    vi.mocked(lock).mockClear();
    vi.mocked(syncRepository.getMetadata).mockResolvedValue(undefined as never);
    settings.set(DEFAULT_SETTINGS);
    isLocking.set(false);
    isLocked.set(false);
    stop = listenForLockElsewhere();
  });

  afterEach(() => {
    stop();
    vi.unstubAllGlobals();
    isLocked.set(false);
  });

  it('tells the other tabs, once', async () => {
    await lockApplication();

    expect(posted).toEqual(['lock']);
  });

  it('locks this tab when told to, saving first, and tells nobody', async () => {
    const save = vi.fn(async () => undefined);
    const stopSave = registerPendingSave(save);

    lockElsewhere();
    await vi.waitFor(() => expect(get(isLocked)).toBe(true));
    stopSave();

    expect(save).toHaveBeenCalled();
    expect(lock).toHaveBeenCalledOnce();
    expect(posted).toEqual([]);
  });

  it('has nothing to do if this tab is already locked', async () => {
    isLocked.set(true);

    lockElsewhere();
    await Promise.resolve();

    expect(lock).not.toHaveBeenCalled();
  });

  it('keeps each notebook to itself', () => {
    expect(FakeChannel.names.at(-1)).toMatch(/^jottery-lock:/);
  });

  it('still locks in a browser without BroadcastChannel', async () => {
    stop();
    vi.stubGlobal('BroadcastChannel', undefined);

    await lockApplication();

    expect(lock).toHaveBeenCalledOnce();
    stop = () => undefined;
  });
});

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
