import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup, waitFor, within } from '@testing-library/svelte';
import { addMessages, init } from 'svelte-i18n';
import en from '../../locales/en-GB.json';
import Account from './Account.svelte';
import { toast } from '../../lib/toast.svelte';

const inbox = en.userPortal.account.inbox;

const mocks = vi.hoisted(() => ({
  hasToken: false,
  generateInboxToken: vi.fn(),
  revokeInboxToken: vi.fn(),
}));

vi.mock('../../lib/userApi', async () => {
  const { ApiError } = await vi.importActual<typeof import('../../lib/dashboardClient')>('../../lib/dashboardClient');
  return {
    ApiError,
    userApi: {
      getAccountInfo: vi.fn(async () => ({
        email: 'me@example.com',
        noteCount: 0,
        attachmentCount: 0,
        storageUsedBytes: 0,
        storageQuotaMb: 100,
        createdAt: '2026-01-02T03:04:05Z',
        lastSyncAt: null,
        inbox: { itemCount: 0, totalSizeBytes: 0, maxItems: 100, maxSizeMb: 10, hasToken: mocks.hasToken },
      })),
      getDevices: vi.fn(async () => []),
      generateInboxToken: mocks.generateInboxToken,
      revokeInboxToken: mocks.revokeInboxToken,
    },
  };
});

function stubClipboard(clipboard: unknown) {
  Object.defineProperty(navigator, 'clipboard', { value: clipboard, configurable: true });
}

function lastToast() {
  return toast.items[toast.items.length - 1];
}

async function revoke() {
  await fireEvent.click(await screen.findByRole('button', { name: inbox.revoke }));
  const dialog = screen.getByRole('dialog', { name: inbox.revokeConfirmTitle });
  await fireEvent.click(within(dialog).getByRole('button', { name: inbox.revoke }));
}

async function generate() {
  mocks.generateInboxToken.mockResolvedValueOnce({ token: 'the-one-time-token' });
  await fireEvent.click(await screen.findByRole('button', { name: inbox.generate }));
  await screen.findByText('the-one-time-token');
}

describe('Account inbox token', () => {
  beforeAll(async () => {
    addMessages('en-GB', en);
    await init({ fallbackLocale: 'en-GB', initialLocale: 'en-GB' });
  });

  beforeEach(() => {
    mocks.hasToken = false;
    vi.clearAllMocks();
  });

  afterEach(() => {
    toast.items.forEach((item) => toast.remove(item.id));
    stubClipboard(undefined);
    cleanup();
  });

  it('says what counts towards storage', async () => {
    render(Account);
    expect(await screen.findByText(en.userPortal.account.storageUsedNote)).toBeTruthy();
  });

  it('confirms a new token', async () => {
    render(Account);
    await generate();
    expect(lastToast()).toMatchObject({ type: 'success', message: inbox.generateSuccess });
  });

  it('says when a token couldn\'t be generated, and keeps offering to', async () => {
    mocks.generateInboxToken.mockRejectedValueOnce(new TypeError('offline'));
    render(Account);

    await fireEvent.click(await screen.findByRole('button', { name: inbox.generate }));

    await waitFor(() => expect(lastToast()?.type).toBe('error'));
    expect(lastToast().message).toContain(en.errors.network);
    expect(screen.getByRole('button', { name: inbox.generate })).toBeTruthy();
  });

  it('confirms a revoked token', async () => {
    mocks.hasToken = true;
    mocks.revokeInboxToken.mockResolvedValueOnce(undefined);
    render(Account);

    await revoke();

    await waitFor(() => expect(lastToast()).toMatchObject({ type: 'success', message: inbox.revokeSuccess }));
  });

  it('warns that a token still works when revoking it failed', async () => {
    mocks.hasToken = true;
    mocks.revokeInboxToken.mockRejectedValueOnce(new TypeError('offline'));
    render(Account);

    await revoke();

    await waitFor(() => expect(lastToast()?.type).toBe('error'));
    expect(lastToast().message).toContain(en.errors.network);
    expect(screen.getByText(inbox.active)).toBeTruthy();
  });

  it('confirms a copy', async () => {
    stubClipboard({ writeText: vi.fn(async () => {}) });
    render(Account);
    await generate();

    await fireEvent.click(screen.getByRole('button', { name: inbox.copyToken }));

    await waitFor(() => expect(lastToast()).toMatchObject({ type: 'success', message: inbox.copySuccess }));
  });

  it('tells people to copy by hand on plain HTTP, instead of doing nothing', async () => {
    stubClipboard(undefined);
    render(Account);
    await generate();

    await fireEvent.click(screen.getByRole('button', { name: inbox.copyToken }));

    await waitFor(() => expect(lastToast()?.message).toBe(inbox.copyUnavailable));
    expect(screen.getByText('the-one-time-token')).toBeTruthy();
  });
});
