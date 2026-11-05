import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

vi.mock('./initService', () => ({ lock: vi.fn() }));
vi.mock('./settingsRepository', () => ({ settingsRepository: { get: vi.fn() } }));
vi.mock('./passwordStorageService', () => ({ hasStoredPassword: vi.fn() }));
vi.mock('./sessionStorageService', () => ({ hasStoredSession: vi.fn(() => true), hasValidSession: vi.fn(() => true) }));

import { startAutoLock, stopAutoLock } from './autoLockService';
import { lock } from './initService';
import { settingsRepository } from './settingsRepository';
import { hasStoredPassword } from './passwordStorageService';

describe('auto-lock with a stored password', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.mocked(hasStoredPassword).mockResolvedValue(true);
  });

  afterEach(() => {
    stopAutoLock();
    vi.useRealTimers();
  });

  const settle = () => vi.advanceTimersByTimeAsync(3 * 60 * 1000);

  it('locks when Remember Password is off, because the password is only left over', async () => {
    vi.mocked(settingsRepository.get).mockResolvedValue({ rememberPassword: false } as never);
    startAutoLock(1);

    await settle();

    expect(lock).toHaveBeenCalled();
  });

  it('does not lock when Remember Password is on, because it would unlock straight away', async () => {
    vi.mocked(lock).mockClear();
    vi.mocked(settingsRepository.get).mockResolvedValue({ rememberPassword: true } as never);
    startAutoLock(1);

    await settle();

    expect(lock).not.toHaveBeenCalled();
  });
});

describe('the idle timer', () => {
  const MINUTE = 60 * 1000;

  beforeEach(() => {
    vi.useFakeTimers();
    vi.mocked(lock).mockClear();
    vi.mocked(hasStoredPassword).mockResolvedValue(false);
    vi.mocked(settingsRepository.get).mockResolvedValue({ rememberPassword: false } as never);
  });

  afterEach(() => {
    stopAutoLock();
    vi.useRealTimers();
  });

  const idle = (minutes: number) => vi.advanceTimersByTimeAsync(minutes * MINUTE);
  const press = (key: string) => document.dispatchEvent(new KeyboardEvent('keydown', { key }));

  it('locks a session that would survive a reload, because that is all it is for', async () => {
    vi.mocked(settingsRepository.get).mockResolvedValue({ rememberPassword: false, persistSession: true, persistSessionTimeout: 30 } as never);
    startAutoLock(5);

    await idle(6);

    expect(lock).toHaveBeenCalled();
  });

  it('locks close to the time set, not up to a minute late', async () => {
    startAutoLock(5);
    await idle(1.5);
    press('a');

    await idle(5.25);

    expect(lock).toHaveBeenCalled();
  });

  it('counts a key that types nothing, such as an arrow', async () => {
    startAutoLock(5);
    await idle(4);

    press('ArrowDown');
    await idle(4);

    expect(lock).not.toHaveBeenCalled();
  });

  it('does not count the mouse drifting across the screen', async () => {
    startAutoLock(5);
    await idle(4);

    document.dispatchEvent(new MouseEvent('mousemove'));
    await idle(2);

    expect(lock).toHaveBeenCalled();
  });

  it('counts activity that something else stops from bubbling', async () => {
    const field = document.createElement('div');
    field.addEventListener('keydown', event => event.stopPropagation());
    document.body.appendChild(field);
    startAutoLock(5);
    await idle(4);

    field.dispatchEvent(new KeyboardEvent('keydown', { key: 'a', bubbles: true }));
    await idle(4);
    field.remove();

    expect(lock).not.toHaveBeenCalled();
  });
});

