import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup, within, waitFor } from '@testing-library/svelte';
import { addMessages, init, locale } from 'svelte-i18n';
import en from '../locales/en-GB.json';
import de from '../locales/de.json';
import Users from './Users.svelte';
import { toast } from '../lib/toast.svelte';
import type { UserListItem } from '../lib/api';

function user(overrides: Partial<UserListItem>): UserListItem {
  return {
    id: 'id',
    email: 'someone@example.com',
    approved: true,
    isAdmin: false,
    isActive: true,
    createdAt: '2026-03-04T05:06:07Z',
    deviceCount: 2,
    noteCount: 37,
    ...overrides,
  };
}

const ME = 'me@example.com';
const OTHER = 'other@example.com';

const mocks = vi.hoisted(() => ({
  users: [] as unknown[],
  signedIn: { email: 'me@example.com' } as { email: string } | null,
  deleteUser: vi.fn(async () => {}),
  deactivateUser: vi.fn(async () => {}),
  approveUser: vi.fn(async () => {}),
  toggleAdmin: vi.fn(async () => {}),
}));

vi.mock('../lib/api', async () => {
  const { ApiError } = await vi.importActual<typeof import('../lib/dashboardClient')>('../lib/dashboardClient');
  return {
    ApiError,
    api: {
      listUsers: vi.fn(async () => ({ users: mocks.users, total: mocks.users.length })),
      deleteUser: mocks.deleteUser,
      deactivateUser: mocks.deactivateUser,
      approveUser: mocks.approveUser,
      toggleAdmin: mocks.toggleAdmin,
    },
  };
});

vi.mock('../lib/auth.svelte', () => ({
  auth: {
    get user() {
      return mocks.signedIn;
    },
  },
}));

async function rowOf(email: string) {
  const link = await screen.findByRole('button', { name: email });
  return within(link.closest('tr') as HTMLElement);
}

describe('Users', () => {
  beforeAll(async () => {
    addMessages('en-GB', en);
    addMessages('de', de);
    await init({ fallbackLocale: 'en-GB', initialLocale: 'en-GB' });
  });

  beforeEach(async () => {
    await locale.set('en-GB');
    mocks.signedIn = { email: ME };
    mocks.users = [
      user({ id: 'me', email: ME, isAdmin: true }),
      user({ id: 'other', email: OTHER, isAdmin: true }),
      user({ id: 'plain', email: 'plain@example.com' }),
      user({ id: 'new', email: 'new@example.com', approved: false }),
    ];
    vi.clearAllMocks();
  });

  afterEach(() => {
    toast.items.forEach((item) => toast.remove(item.id));
    cleanup();
  });

  describe('protecting the signed-in admin', () => {
    it('offers nothing on their own row that would lock them out', async () => {
      render(Users);
      const row = await rowOf(ME);

      expect(row.queryByRole('button', { name: en.users.actions.deactivate })).toBeNull();
      expect(row.queryByRole('button', { name: en.users.actions.delete })).toBeNull();
      expect(row.queryByRole('button', { name: en.users.actions.removeAdmin })).toBeNull();
    });

    it('still offers them on another admin\'s row', async () => {
      render(Users);
      const row = await rowOf(OTHER);

      expect(row.getByRole('button', { name: en.users.actions.deactivate })).toBeTruthy();
      expect(row.getByRole('button', { name: en.users.actions.delete })).toBeTruthy();
      expect(row.getByRole('button', { name: en.users.actions.removeAdmin })).toBeTruthy();
    });

    it('ignores case when matching the signed-in address', async () => {
      mocks.signedIn = { email: ME.toUpperCase() };
      render(Users);
      const row = await rowOf(ME);

      expect(row.queryByRole('button', { name: en.users.actions.deactivate })).toBeNull();
    });

    it('protects the last active admin even when it can\'t tell who is signed in', async () => {
      mocks.signedIn = null;
      mocks.users = [user({ id: 'me', email: ME, isAdmin: true }), user({ id: 'plain', email: 'plain@example.com' })];
      render(Users);
      const row = await rowOf(ME);

      expect(row.queryByRole('button', { name: en.users.actions.deactivate })).toBeNull();
      expect(row.queryByRole('button', { name: en.users.actions.delete })).toBeNull();
    });

    it('doesn\'t count an inactive admin as a spare', async () => {
      mocks.signedIn = null;
      mocks.users = [user({ id: 'me', email: ME, isAdmin: true }), user({ id: 'gone', email: OTHER, isAdmin: true, isActive: false })];
      render(Users);
      const row = await rowOf(ME);

      expect(row.queryByRole('button', { name: en.users.actions.delete })).toBeNull();
    });
  });

  describe('deleting a user', () => {
    async function openDelete(email = 'plain@example.com') {
      render(Users);
      const row = await rowOf(email);
      await fireEvent.click(row.getByRole('button', { name: en.users.actions.delete }));
      return screen.getByRole('dialog');
    }

    function confirmButton(dialog: HTMLElement) {
      return within(dialog).getByRole('button', { name: en.users.confirm.delete }) as HTMLButtonElement;
    }

    it('is a red dialog', async () => {
      const dialog = await openDelete();
      expect(confirmButton(dialog).className).toContain('bg-red-600');
    });

    it('says what goes with the account, and how much', async () => {
      const dialog = await openDelete();
      expect(dialog.textContent).toContain('attachments');
      expect(dialog.textContent).toContain('account key');
      expect(dialog.textContent).toContain('37');
      expect(dialog.textContent).toContain('2');
    });

    it('needs the address typed before anything is deleted', async () => {
      const dialog = await openDelete();
      expect(confirmButton(dialog).disabled).toBe(true);

      const field = within(dialog).getByLabelText(en.users.confirm.typeEmail);
      await fireEvent.input(field, { target: { value: 'plain@example.com' } });
      await fireEvent.click(confirmButton(dialog));

      await waitFor(() => expect(mocks.deleteUser).toHaveBeenCalledWith('plain'));
    });
  });

  it('warns in red before deactivating or demoting', async () => {
    render(Users);
    const row = await rowOf(OTHER);

    await fireEvent.click(row.getByRole('button', { name: en.users.actions.deactivate }));
    let confirm = within(screen.getByRole('dialog')).getByRole('button', { name: en.users.confirm.deactivate });
    expect(confirm.className).toContain('bg-red-600');
    await fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: en.common.cancel }));

    await fireEvent.click(row.getByRole('button', { name: en.users.actions.removeAdmin }));
    confirm = within(screen.getByRole('dialog')).getByRole('button', { name: en.users.confirm.demoteFromAdmin });
    expect(confirm.className).toContain('bg-red-600');
  });

  it('keeps a routine approval blue', async () => {
    render(Users);
    const row = await rowOf('new@example.com');

    await fireEvent.click(row.getByRole('button', { name: en.users.actions.approve }));

    const confirm = within(screen.getByRole('dialog')).getByRole('button', { name: en.users.confirm.approve });
    expect(confirm.className).toContain('bg-blue-600');
  });

  describe('in another language', () => {
    beforeEach(async () => {
      await locale.set('de');
    });

    it('never puts an English action name in a sentence', async () => {
      render(Users);
      const row = await rowOf('new@example.com');

      await fireEvent.click(row.getByRole('button', { name: de.users.actions.approve }));

      const dialog = screen.getByRole('dialog');
      expect(dialog.textContent).toContain(de.users.confirm.messages.approve.replace('{email}', 'new@example.com'));
      expect(dialog.textContent).not.toMatch(/\bapprove\b/);
    });

    it('reports a failure without the internal action name', async () => {
      mocks.toggleAdmin.mockRejectedValueOnce(new Error('offline'));
      render(Users);
      const row = await rowOf('plain@example.com');

      await fireEvent.click(row.getByRole('button', { name: de.users.actions.makeAdmin }));
      await fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: de.users.confirm.promoteToAdmin }));

      await waitFor(() => expect(toast.items.length).toBe(1));
      expect(toast.items[0].message).not.toContain('toggleAdmin');
      expect(toast.items[0].message).toContain(de.users.toast.failed.promote.split('{error}')[0].trim());
    });

    it('formats dates for the chosen language, not always en-GB', async () => {
      render(Users);
      await rowOf(ME);

      const expected = new Date('2026-03-04T05:06:07Z').toLocaleDateString('de');
      expect(screen.getAllByText(expected).length).toBeGreaterThan(0);
    });
  });
});
