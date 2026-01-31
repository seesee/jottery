import './app.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { initI18n, getInitialLocale } from './lib/services/i18nService';

// Get stored language preference, or detect from browser
const LANGUAGE_STORAGE_KEY = 'admin_language';
const storedLanguage = localStorage.getItem(LANGUAGE_STORAGE_KEY);
const initialLocale = getInitialLocale(storedLanguage || undefined);

// Initialize internationalisation and mount app after it's ready
initI18n(initialLocale).then(() => {
  const app = mount(App, {
    target: document.getElementById('app')!,
  });
});

export default null;
