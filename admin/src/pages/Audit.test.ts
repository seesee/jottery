import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup, within, waitFor } from '@testing-library/svelte';
import { addMessages, init, locale } from 'svelte-i18n';
import en from '../locales/en-GB.json';
import de from '../locales/de.json';
import Audit from './Audit.svelte';
import { AUDIT_ACTIONS } from '../lib/auditActions';

const mocks = vi.hoisted(() => ({
  getAuditLog: vi.fn(),
  listUsers: vi.fn(),
}));

vi.mock('../lib/api', async () => {
  const { ApiError } = await vi.importActual<typeof import('../lib/dashboardClient')>('../lib/dashboardClient');
  return { ApiError, api: { getAuditLog: mocks.getAuditLog, listUsers: mocks.listUsers } };
});

// The filter lists every action too, so anything about the log has to look in the table
async function inTable(text: string) {
  return within(await screen.findByRole('table')).findByText(text);
}

function entry(overrides: Record<string, unknown> = {}) {
  return {
    id: crypto.randomUUID(),
    adminUserId: 'admin-1',
    action: 'user.approve',
    targetUserId: 'user-1',
    targetResourceId: null,
    createdAt: '2026-03-04T05:06:07+00:00',
    ipAddress: '203.0.113.7',
    userAgent: 'Mozilla/5.0',
    metadata: null,
    ...overrides,
  };
}

describe('Audit', () => {
  beforeAll(async () => {
    addMessages('en-GB', en);
    addMessages('de', de);
    await init({ fallbackLocale: 'en-GB', initialLocale: 'en-GB' });
  });

  beforeEach(async () => {
    await locale.set('en-GB');
    mocks.getAuditLog.mockReset();
    mocks.listUsers.mockReset();
    mocks.listUsers.mockResolvedValue({
      users: [
        { id: 'admin-1', email: 'admin@example.com' },
        { id: 'user-1', email: 'someone@example.com' },
      ],
      total: 2,
    });
    mocks.getAuditLog.mockResolvedValue({ logs: [entry()], total: 1 });
  });

  afterEach(cleanup);

  it('has a translation for every action the server records', () => {
    for (const key of Object.values(AUDIT_ACTIONS)) {
      expect(key.startsWith('audit.actions.')).toBe(true);
      const name = key.replace('audit.actions.', '') as keyof typeof en.audit.actions;
      expect(typeof en.audit.actions[name], key).toBe('string');
    }
  });

  it('shows what happened in words, with addresses instead of ids', async () => {
    render(Audit);

    const row = (await inTable(en.audit.actions.userApprove)).closest('tr') as HTMLElement;
    expect(within(row).getByText('admin@example.com')).toBeTruthy();
    expect(within(row).getByText('someone@example.com')).toBeTruthy();
    expect(within(row).getByText('203.0.113.7')).toBeTruthy();
  });

  it('uses the chosen language for actions and dates', async () => {
    await locale.set('de');
    render(Audit);

    expect(await inTable(de.audit.actions.userApprove)).toBeTruthy();
    const expected = new Date('2026-03-04T05:06:07+00:00').toLocaleString('de');
    expect(screen.getByText(expected)).toBeTruthy();
  });

  it('names the command line when no admin account was involved', async () => {
    mocks.getAuditLog.mockResolvedValue({ logs: [entry({ adminUserId: null })], total: 1 });
    render(Audit);
    expect(await inTable(en.audit.commandLine)).toBeTruthy();
  });

  it('still shows an entry for an account that has since been deleted', async () => {
    mocks.getAuditLog.mockResolvedValue({
      logs: [entry({ action: 'user.delete', targetUserId: 'abcdef12-3456-7890-abcd-ef1234567890' })],
      total: 1,
    });
    render(Audit);

    const shown = await inTable('abcdef12…');
    expect(shown.getAttribute('title')).toBe('abcdef12-3456-7890-abcd-ef1234567890');
  });

  it('shows an action added to a newer server as it stands', async () => {
    mocks.getAuditLog.mockResolvedValue({ logs: [entry({ action: 'user.something_new' })], total: 1 });
    render(Audit);
    expect(await inTable('user.something_new')).toBeTruthy();
  });

  it('shows extra detail as text, so stored markup stays inert', async () => {
    const hostile = '<img src=x onerror=alert(1)>';
    mocks.getAuditLog.mockResolvedValue({ logs: [entry({ metadata: { note: hostile } })], total: 1 });
    const { container } = render(Audit);

    await inTable(en.audit.actions.userApprove);

    expect(container.querySelector('img')).toBeNull();
    expect(container.textContent).toContain('onerror');
  });

  it('filters by action on the server', async () => {
    render(Audit);
    await inTable(en.audit.actions.userApprove);

    await fireEvent.change(screen.getByLabelText(en.audit.filterLabel), { target: { value: 'device.revoke' } });

    await waitFor(() =>
      expect(mocks.getAuditLog).toHaveBeenLastCalledWith({ limit: 50, offset: 0, action: 'device.revoke' }),
    );
  });

  it('pages through the log, and stops at each end', async () => {
    mocks.getAuditLog.mockResolvedValue({ logs: Array.from({ length: 50 }, () => entry()), total: 120 });
    render(Audit);
    await screen.findByText(en.audit.range.replace('{from}', '1').replace('{to}', '50').replace('{total}', '120'));

    const previous = screen.getByRole('button', { name: en.audit.previous }) as HTMLButtonElement;
    const next = screen.getByRole('button', { name: en.audit.next }) as HTMLButtonElement;
    expect(previous.disabled).toBe(true);

    await fireEvent.click(next);
    await waitFor(() => expect(mocks.getAuditLog).toHaveBeenLastCalledWith({ limit: 50, offset: 50 }));

    mocks.getAuditLog.mockResolvedValue({ logs: Array.from({ length: 20 }, () => entry()), total: 120 });
    await fireEvent.click(next);
    await waitFor(() => expect(mocks.getAuditLog).toHaveBeenLastCalledWith({ limit: 50, offset: 100 }));
    await screen.findByText(en.audit.range.replace('{from}', '101').replace('{to}', '120').replace('{total}', '120'));
    expect(next.disabled).toBe(true);
  });

  it('goes back to the first page when the filter changes', async () => {
    mocks.getAuditLog.mockResolvedValue({ logs: Array.from({ length: 50 }, () => entry()), total: 120 });
    render(Audit);
    await screen.findByRole('table');
    await fireEvent.click(screen.getByRole('button', { name: en.audit.next }));
    await waitFor(() => expect(mocks.getAuditLog).toHaveBeenLastCalledWith({ limit: 50, offset: 50 }));

    await fireEvent.change(screen.getByLabelText(en.audit.filterLabel), { target: { value: 'user.delete' } });

    await waitFor(() =>
      expect(mocks.getAuditLog).toHaveBeenLastCalledWith({ limit: 50, offset: 0, action: 'user.delete' }),
    );
  });

  it('says so when the log is empty', async () => {
    mocks.getAuditLog.mockResolvedValue({ logs: [], total: 0 });
    render(Audit);
    expect(await screen.findByText(en.audit.empty)).toBeTruthy();
  });

  it('reports a failure', async () => {
    mocks.getAuditLog.mockRejectedValue(new TypeError('offline'));
    render(Audit);
    expect((await screen.findByRole('alert')).textContent).toContain(en.errors.network);
  });

  it('still shows the log if the user list can\'t be read', async () => {
    mocks.listUsers.mockRejectedValue(new TypeError('offline'));
    render(Audit);
    expect(await inTable(en.audit.actions.userApprove)).toBeTruthy();
    expect(screen.queryByRole('alert')).toBeNull();
  });
});
