import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup, within } from '@testing-library/svelte';
import { addMessages, init } from 'svelte-i18n';
import en from '../../locales/en-GB.json';
import Login from './Login.svelte';

const login = en.userPortal.login;

const mocks = vi.hoisted(() => ({ checkStatus: vi.fn() }));

vi.mock('../../lib/userApi', async () => {
  const { ApiError } = await vi.importActual<typeof import('../../lib/dashboardClient')>('../../lib/dashboardClient');
  return { ApiError, userApi: { checkStatus: mocks.checkStatus } };
});

vi.mock('../../lib/userAuth.svelte', () => ({ userAuth: { sessionExpired: false, login: vi.fn() } }));

async function openStatusCheck() {
  render(Login);
  await fireEvent.click(screen.getByRole('button', { name: login.checkStatus }));
  const section = screen.getByRole('heading', { name: login.statusCheck.title }).parentElement as HTMLElement;
  return {
    section,
    email: within(section).getByRole('textbox') as HTMLInputElement,
    password: section.querySelector('input[type="password"]') as HTMLInputElement,
    button: within(section).getByRole('button', { name: login.statusCheck.checkButton }) as HTMLButtonElement,
  };
}

async function check(form: Awaited<ReturnType<typeof openStatusCheck>>) {
  await fireEvent.input(form.email, { target: { value: 'me@example.com' } });
  await fireEvent.input(form.password, { target: { value: 'the-password' } });
  await fireEvent.submit(form.button.closest('form')!);
}

describe('Portal status check', () => {
  beforeAll(async () => {
    addMessages('en-GB', en);
    await init({ fallbackLocale: 'en-GB', initialLocale: 'en-GB' });
  });

  beforeEach(() => {
    mocks.checkStatus.mockReset();
  });

  afterEach(cleanup);

  it('needs the password as well as the address', async () => {
    const form = await openStatusCheck();
    expect(form.password.autocomplete).toBe('current-password');
    expect(form.password.getAttribute('aria-label')).toBe(login.password);

    await fireEvent.input(form.email, { target: { value: 'me@example.com' } });
    expect(form.button.disabled).toBe(true);

    await fireEvent.input(form.password, { target: { value: 'x' } });
    expect(form.button.disabled).toBe(false);
  });

  it('sends both, and shows where the account stands', async () => {
    mocks.checkStatus.mockResolvedValueOnce({ exists: true, isApproved: false, isActive: true });
    const form = await openStatusCheck();

    await check(form);

    expect(await within(form.section).findByText(login.statusCheck.pending)).toBeTruthy();
    expect(mocks.checkStatus).toHaveBeenCalledWith('me@example.com', 'the-password');
  });

  it('doesn\'t say whether the address exists when the password is wrong', async () => {
    const { ApiError } = await import('../../lib/dashboardClient');
    mocks.checkStatus.mockRejectedValueOnce(new ApiError(401, 'Unauthorized'));
    const form = await openStatusCheck();

    await check(form);

    const alert = await within(form.section).findByRole('alert');
    expect(alert.textContent).toContain(login.errors.invalidCredentials);
  });

  it('asks people to wait when rate limited', async () => {
    const { ApiError } = await import('../../lib/dashboardClient');
    mocks.checkStatus.mockRejectedValueOnce(new ApiError(429, 'Too many', 'rate_limited', 30));
    const form = await openStatusCheck();

    await check(form);

    const alert = await within(form.section).findByRole('alert');
    expect(alert.textContent).toContain('30');
  });
});
