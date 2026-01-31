import './app.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { initI18n } from './lib/services/i18nService';

// Get stored language preference
const LANGUAGE_STORAGE_KEY = 'admin_language';
const storedLanguage = localStorage.getItem(LANGUAGE_STORAGE_KEY);

// Initialize internationalisation and mount app after it's ready
initI18n(storedLanguage || undefined).then(() => {
  const app = mount(App, {
    target: document.getElementById('app')!,
  });
});

export default null;
