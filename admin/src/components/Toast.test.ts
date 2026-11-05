import { describe, it, expect, beforeAll, afterEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/svelte';
import { addMessages, init } from 'svelte-i18n';
import en from '../locales/en-GB.json';
import Toast from './Toast.svelte';
import { toast } from '../lib/toast.svelte';

describe('Toast', () => {
  beforeAll(async () => {
    addMessages('en-GB', en);
    await init({ fallbackLocale: 'en-GB', initialLocale: 'en-GB' });
  });

  afterEach(() => {
    toast.items.forEach((item) => toast.remove(item.id));
    cleanup();
  });

  it('announces success politely', async () => {
    render(Toast);
    toast.success('Saved');
    await waitFor(() => expect(screen.getByRole('status').textContent).toContain('Saved'));
  });

  it('announces failure at once', async () => {
    render(Toast);
    toast.error('It failed');
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('It failed'));
  });

  it('keeps a live region on the page before any toast appears', () => {
    const { container } = render(Toast);
    expect(container.querySelector('[aria-live]')).not.toBeNull();
  });

  it('names its close button', async () => {
    render(Toast);
    toast.info('Hello');
    await waitFor(() => expect(screen.getByRole('button', { name: en.common.close })).toBeTruthy());
  });
});
