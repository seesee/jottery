import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup, waitFor, within } from '@testing-library/svelte';
import { addMessages, init } from 'svelte-i18n';
import en from '../../locales/en-GB.json';
import UserSettings from './UserSettings.svelte';
import { toast } from '../../lib/toast.svelte';

const settings = en.userPortal.settings;
const PASSWORD = 'the-account-password';

const mocks = vi.hoisted(() => ({
  deleteAllNotes: vi.fn(),
  deleteAccount: vi.fn(),
  clearAuth: vi.fn(),
}));

vi.mock('../../lib/userApi', async () => {
  const { ApiError } = await vi.importActual<typeof import('../../lib/dashboardClient')>('../../lib/dashboardClient');
  return { ApiError, userApi: { deleteAllNotes: mocks.deleteAllNotes, deleteAccount: mocks.deleteAccount } };
});

vi.mock('../../lib/userAuth.svelte', () => ({ userAuth: { clearAuth: mocks.clearAuth } }));

async function wrongPassword() {
  const { ApiError } = await import('../../lib/dashboardClient');
  return new ApiError(401, 'Unauthorized');
}

async function openDeleteNotes() {
  render(UserSettings);
  await fireEvent.click(screen.getByRole('button', { name: settings.deleteNotesButton }));
  return screen.getByRole('dialog', { name: settings.deleteNotesConfirmTitle });
}

async function openDeleteAccount() {
  render(UserSettings);
  await fireEvent.click(screen.getByRole('button', { name: settings.delete }));
  return screen.getByRole('dialog', { name: settings.deleteConfirmTitle });
}

describe('UserSettings danger zone', () => {
  beforeAll(async () => {
    addMessages('en-GB', en);
    await init({ fallbackLocale: 'en-GB', initialLocale: 'en-GB' });
  });

  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal('localStorage', { getItem: () => null, setItem: () => {} });
  });

  afterEach(() => {
    toast.items.forEach((item) => toast.remove(item.id));
    vi.unstubAllGlobals();
    cleanup();
  });

  describe('deleting all notes', () => {
    function button(dialog: HTMLElement) {
      return within(dialog).getByRole('button', { name: settings.deleteNotesButton }) as HTMLButtonElement;
    }

    it('needs the password, and sends it', async () => {
      mocks.deleteAllNotes.mockResolvedValueOnce(undefined);
      const dialog = await openDeleteNotes();
      expect(button(dialog).disabled).toBe(true);

      await fireEvent.input(within(dialog).getByLabelText(settings.passwordToConfirm), { target: { value: PASSWORD } });
      await fireEvent.click(button(dialog));

      await waitFor(() => expect(mocks.deleteAllNotes).toHaveBeenCalledWith(PASSWORD));
      await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    });

    it('stays open and says so when the password is wrong', async () => {
      mocks.deleteAllNotes.mockRejectedValueOnce(await wrongPassword());
      const dialog = await openDeleteNotes();

      await fireEvent.input(within(dialog).getByLabelText(settings.passwordToConfirm), { target: { value: 'nope' } });
      await fireEvent.click(button(dialog));

      const alert = await within(dialog).findByRole('alert');
      expect(alert.textContent).toContain(en.errors.wrongPassword);
      expect(screen.getByRole('dialog')).toBeTruthy();
      expect(toast.items.length).toBe(0);
    });
  });

  describe('deleting the account', () => {
    function button(dialog: HTMLElement) {
      return within(dialog).getByRole('button', { name: settings.deleteForever }) as HTMLButtonElement;
    }

    async function fill(dialog: HTMLElement, typed: string, password: string) {
      await fireEvent.input(within(dialog).getByLabelText(settings.deleteConfirmLabel), { target: { value: typed } });
      await fireEvent.input(within(dialog).getByLabelText(settings.passwordToConfirm), { target: { value: password } });
    }

    it('needs both the word and the password', async () => {
      const dialog = await openDeleteAccount();

      await fill(dialog, 'DELETE', '');
      expect(button(dialog).disabled).toBe(true);

      await fill(dialog, 'delete', PASSWORD);
      expect(button(dialog).disabled).toBe(true);

      await fill(dialog, 'DELETE', PASSWORD);
      expect(button(dialog).disabled).toBe(false);
    });

    it('sends the password and signs out', async () => {
      mocks.deleteAccount.mockResolvedValueOnce(undefined);
      const dialog = await openDeleteAccount();

      await fill(dialog, 'DELETE', PASSWORD);
      await fireEvent.click(button(dialog));

      await waitFor(() => expect(mocks.deleteAccount).toHaveBeenCalledWith(PASSWORD));
      await waitFor(() => expect(mocks.clearAuth).toHaveBeenCalledOnce());
    });

    it('keeps the session when the password is wrong', async () => {
      mocks.deleteAccount.mockRejectedValueOnce(await wrongPassword());
      const dialog = await openDeleteAccount();

      await fill(dialog, 'DELETE', 'nope');
      await fireEvent.click(button(dialog));

      const alert = await within(dialog).findByRole('alert');
      expect(alert.textContent).toContain(en.errors.wrongPassword);
      expect(mocks.clearAuth).not.toHaveBeenCalled();
    });
  });
});
