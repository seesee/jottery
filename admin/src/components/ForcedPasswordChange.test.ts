import { describe, it, expect, vi, beforeAll, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup, waitFor } from '@testing-library/svelte';
import { addMessages, init } from 'svelte-i18n';
import en from '../locales/en-GB.json';
import ForcedPasswordChange from './ForcedPasswordChange.svelte';
import { ApiError } from '../lib/dashboardClient';
import type { DashboardAuth } from '../lib/dashboardAuth.svelte';

const CURRENT = 'the-current-password';
const NEXT = 'a-brand-new-password';

function fakeAuth(overrides: Record<string, unknown> = {}) {
  return {
    user: { email: 'admin@example.com' },
    changePassword: vi.fn(async () => {}),
    logout: vi.fn(async () => {}),
    ...overrides,
  } as unknown as DashboardAuth;
}

function showScreen(auth: DashboardAuth) {
  render(ForcedPasswordChange, { props: { auth } });
  return {
    current: screen.getByLabelText(en.settings.changePassword.currentPassword) as HTMLInputElement,
    next: screen.getByLabelText(en.settings.changePassword.newPassword) as HTMLInputElement,
    confirm: screen.getByLabelText(en.settings.changePassword.confirmPassword) as HTMLInputElement,
    submit: screen.getByRole('button', { name: en.settings.changePassword.button }),
  };
}

async function fill(form: ReturnType<typeof showScreen>, current: string, next: string, confirm: string) {
  await fireEvent.input(form.current, { target: { value: current } });
  await fireEvent.input(form.next, { target: { value: next } });
  await fireEvent.input(form.confirm, { target: { value: confirm } });
  await fireEvent.submit(form.submit.closest('form')!);
}

describe('ForcedPasswordChange', () => {
  beforeAll(async () => {
    addMessages('en-GB', en);
    await init({ fallbackLocale: 'en-GB', initialLocale: 'en-GB' });
  });

  afterEach(cleanup);

  it('says the rules go beyond length, and explains a refusal', async () => {
    const auth = fakeAuth({
      changePassword: vi.fn(async () => {
        throw new ApiError(400, 'Password requires at least 3 of 4 character types. Missing: number');
      }),
    });
    const form = showScreen(auth);
    expect(screen.getByText(en.common.passwordRules)).toBeTruthy();

    await fill(form, CURRENT, NEXT, NEXT);

    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain(en.errors.passwordClass.number);
  });

  it('explains itself and says whose account it is', () => {
    showScreen(fakeAuth());

    expect(screen.getByRole('heading', { name: en.forcedPasswordChange.title })).toBeTruthy();
    expect(screen.getByText(en.forcedPasswordChange.description)).toBeTruthy();
    expect(screen.getByText('admin@example.com')).toBeTruthy();
  });

  it('works without knowing the email, as after a reload', () => {
    showScreen(fakeAuth({ user: null }));

    expect(screen.getByRole('heading', { name: en.forcedPasswordChange.title })).toBeTruthy();
  });

  it('starts on the first field', () => {
    const form = showScreen(fakeAuth());

    expect(document.activeElement).toBe(form.current);
  });

  it('tells a password manager which field is which', () => {
    const form = showScreen(fakeAuth());

    expect(form.current.autocomplete).toBe('current-password');
    expect(form.next.autocomplete).toBe('new-password');
    expect(form.confirm.autocomplete).toBe('new-password');
  });

  it('changes the password', async () => {
    const auth = fakeAuth();
    const form = showScreen(auth);

    await fill(form, CURRENT, NEXT, NEXT);

    expect(auth.changePassword).toHaveBeenCalledWith(CURRENT, NEXT);
  });

  it.each([
    ['too short', 'short', 'short', en.errors.passwordTooShort],
    ['mismatched', NEXT, `${NEXT}x`, en.settings.changePassword.errors.mismatch],
    ['unchanged', CURRENT, CURRENT, en.errors.passwordSameAsCurrent],
  ])('refuses a %s password without asking the server', async (_case, next, confirm, message) => {
    const auth = fakeAuth();
    const form = showScreen(auth);

    await fill(form, CURRENT, next, confirm);

    expect(auth.changePassword).not.toHaveBeenCalled();
    expect(screen.getByRole('alert').textContent).toContain(message);
  });

  it('says so when the current password is wrong, and stays open', async () => {
    const auth = fakeAuth({ changePassword: vi.fn().mockRejectedValue(new ApiError(401, 'Unauthorized')) });
    const form = showScreen(auth);

    await fill(form, 'not-the-password', NEXT, NEXT);

    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain(en.errors.wrongPassword));
    expect(form.submit).toBeTruthy();
  });

  it('shows something for a failure with no explanation', async () => {
    const auth = fakeAuth({ changePassword: vi.fn().mockRejectedValue(new ApiError(500, '')) });
    const form = showScreen(auth);

    await fill(form, CURRENT, NEXT, NEXT);

    await waitFor(() =>
      expect(screen.getByRole('alert').textContent).toContain(en.settings.changePassword.errors.failed),
    );
  });

  it('lets someone sign out instead', async () => {
    const auth = fakeAuth();
    showScreen(auth);

    await fireEvent.click(screen.getByRole('button', { name: en.nav.logout }));

    expect(auth.logout).toHaveBeenCalled();
  });
});
