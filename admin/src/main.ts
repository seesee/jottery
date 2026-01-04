import './app.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { initI18n } from './lib/services/i18nService';

// Initialize internationalisation
initI18n();

const app = mount(App, {
  target: document.getElementById('app')!,
});

export default app;
