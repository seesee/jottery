import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup, waitFor } from '@testing-library/svelte';
import { addMessages, init, locale } from 'svelte-i18n';
import en from '../locales/en-GB.json';
import de from '../locales/de.json';
import UserDetailModal from './UserDetailModal.svelte';
import { ApiError } from '../lib/dashboardClient';
import { toast } from '../lib/toast.svelte';

const detail = {
  id: 'user-1',
  email: 'someone@example.com',
  approved: true,
  isAdmin: false,
  isActive: true,
  createdAt: '2026-01-02T03:04:05Z',
  approvedAt: null,
  lastLoginAt: null,
  storageQuotaMb: 100,
  maxUploadSizeMb: 10,
  stats: {
    devices: { total: 1, active: 1, lastSeen: null },
    notes: { count: 0 },
    attachments: { count: 0, totalBytes: 0 },
    inbox: { count: 0, totalBytes: 0, hasToken: false },
    lastSyncAt: null,
  },
  devices: [
    { id: 'device-1', name: 'Laptop', type: 'web', createdAt: '2026-01-02T03:04:05Z', lastSeenAt: null, isActive: true },
  ],
};

const mocks = vi.hoisted(() => ({ resetUserPassword: vi.fn() }));

vi.mock('../lib/api', async () => {
  const { ApiError } = await vi.importActual<typeof import('../lib/dashboardClient')>('../lib/dashboardClient');
  return { ApiError, api: { getUser: vi.fn(async () => detail), resetUserPassword: mocks.resetUserPassword } };
});

async function show() {
  const onClose = vi.fn();
  render(UserDetailModal, { props: { userId: 'user-1', onClose } });
  await screen.findByText(detail.email);
  return { onClose };
}

describe('UserDetailModal', () => {
  beforeAll(async () => {
    addMessages('en-GB', en);
    await init({ fallbackLocale: 'en-GB', initialLocale: 'en-GB' });
  });

  beforeEach(async () => {
    await locale.set('en-GB');
  });

  afterEach(cleanup);

  it('formats dates for the chosen language', async () => {
    addMessages('de', de);
    await locale.set('de');

    await show();

    const expected = new Date(detail.createdAt).toLocaleDateString('de', { year: 'numeric', month: 'short', day: 'numeric' });
    expect(screen.getAllByText(expected).length).toBeGreaterThan(0);
  });

  it('is a named dialog that closes on Escape', async () => {
    const { onClose } = await show();
    const dialog = screen.getByRole('dialog', { name: en.users.detail.title });

    await fireEvent.keyDown(dialog, { key: 'Escape' });

    expect(onClose).toHaveBeenCalledOnce();
  });

  it('labels the rename field, and Escape there ends only the rename', async () => {
    const { onClose } = await show();
    await fireEvent.click(screen.getByRole('button', { name: en.devices.rename }));
    const field = screen.getByRole('textbox', { name: en.devices.name });

    await fireEvent.keyDown(field, { key: 'Escape' });

    expect(screen.queryByRole('textbox', { name: en.devices.name })).toBeNull();
    expect(onClose).not.toHaveBeenCalled();
  });

  describe('resetting a password', () => {
    const NEW = 'A-long-replacement-1';

    async function openReset() {
      await show();
      await fireEvent.click(screen.getByRole('button', { name: en.users.detail.resetPassword }));
      const [first, second] = document.querySelectorAll<HTMLInputElement>('input[type="password"]');
      await fireEvent.input(first, { target: { value: NEW } });
      await fireEvent.input(second, { target: { value: NEW } });
    }

    function submit() {
      return fireEvent.click(screen.getByRole('button', { name: en.users.detail.resetPassword }));
    }

    beforeEach(() => {
      mocks.resetUserPassword.mockReset();
      toast.items.forEach((item) => toast.remove(item.id));
    });

    it('says the devices stay connected', async () => {
      await openReset();
      expect(screen.getByText(en.users.detail.passwordResetDevices)).toBeTruthy();
    });

    it('leaves the device keys alone unless asked', async () => {
      mocks.resetUserPassword.mockResolvedValueOnce(undefined);
      await openReset();

      await submit();

      await waitFor(() => expect(mocks.resetUserPassword).toHaveBeenCalledWith('user-1', NEW, false));
    });

    it('can revoke them too', async () => {
      mocks.resetUserPassword.mockResolvedValueOnce(undefined);
      await openReset();

      await fireEvent.click(screen.getByRole('checkbox', { name: en.users.detail.revokeDevicesOption }));
      await submit();

      await waitFor(() => expect(mocks.resetUserPassword).toHaveBeenCalledWith('user-1', NEW, true));
    });

    it('shows why the server refused a password', async () => {
      mocks.resetUserPassword.mockRejectedValueOnce(
        new ApiError(400, 'Password requires at least 3 of 4 character types. Missing: number, special character'),
      );
      await openReset();

      await submit();

      await waitFor(() => expect(toast.items.length).toBe(1));
      expect(toast.items[0].message).toContain(en.errors.passwordClass.number);
      expect(toast.items[0].message).toContain(en.errors.passwordClass.special);
    });
  });

  it('won\'t let the browser fill the admin\'s own password into a reset', async () => {
    await show();
    await fireEvent.click(screen.getByRole('button', { name: en.users.detail.resetPassword }));

    const fields = document.querySelectorAll<HTMLInputElement>('input[type="password"]');
    expect(fields.length).toBe(2);
    fields.forEach((field) => expect(field.autocomplete).toBe('new-password'));
  });
});
