import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from 'vitest';
import { render, cleanup, waitFor } from '@testing-library/svelte';
import { addMessages, init, locale } from 'svelte-i18n';
import en from './locales/en-GB.json';
import de from './locales/de.json';
import App from './App.svelte';

const mode = vi.hoisted(() => ({ portal: false }));

vi.mock('./lib/appMode', () => ({
  get isUserPortal() {
    return mode.portal;
  },
}));
vi.mock('./AdminApp.svelte', () => ({ default: () => {} }));
vi.mock('./UserApp.svelte', () => ({ default: () => {} }));

describe('App', () => {
  beforeAll(async () => {
    addMessages('en-GB', en);
    addMessages('de', de);
    await init({ fallbackLocale: 'en-GB', initialLocale: 'en-GB' });
  });

  beforeEach(async () => {
    mode.portal = false;
    await locale.set('en-GB');
  });

  afterEach(cleanup);

  it('tells screen readers which language the page is in, and follows a change', async () => {
    render(App);
    expect(document.documentElement.lang).toBe('en-GB');

    await locale.set('de');

    await waitFor(() => expect(document.documentElement.lang).toBe('de'));
  });

  it('titles the dashboard in the chosen language', async () => {
    render(App);
    expect(document.title).toBe(en.app.name);

    await locale.set('de');

    await waitFor(() => expect(document.title).toBe(de.app.name));
  });

  it('gives the portal its own title, not the admin one', async () => {
    mode.portal = true;
    render(App);

    expect(document.title).toContain(en.userPortal.title);
    expect(document.title).not.toContain(en.app.name);
  });
});
