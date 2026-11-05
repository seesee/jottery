import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const createBackup = vi.fn();
const createBatchedBackup = vi.fn(async () => ({ version: '3.0' }));
const downloadBackup = vi.fn();

vi.mock('./backupService', () => ({ createBackup, createBatchedBackup, downloadBackup }));
vi.mock('./settingsRepository', () => ({
  settingsRepository: {
    get: vi.fn(async () => ({})),
    update: vi.fn(async () => undefined),
  },
}));

describe('automatic backups', () => {
  beforeEach(() => vi.clearAllMocks());
  afterEach(() => vi.resetModules());

  it('write the current format, because older files lost records on restore', async () => {
    const { triggerAutoBackup } = await import('./backupSchedulerService');

    await triggerAutoBackup();

    expect(createBatchedBackup).toHaveBeenCalledOnce();
    expect(createBackup).not.toHaveBeenCalled();
    expect(downloadBackup).toHaveBeenCalledWith({ version: '3.0' });
  });
});
