import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

vi.mock('./initService', () => ({ lock: vi.fn() }));
vi.mock('./settingsRepository', () => ({ settingsRepository: { get: vi.fn() } }));
vi.mock('./passwordStorageService', () => ({ hasStoredPassword: vi.fn() }));

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
