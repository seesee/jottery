import './app.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { getCurrentNotebook, isNotebookPath } from './lib/utils/notebookPath';
import { setDatabaseName } from './lib/services/db';

// Early notebook detection - configure database before app initialization
const currentPath = window.location.pathname;

let app;

if (!isNotebookPath(currentPath)) {
  // Show error for excluded paths like /admin or /api
  const appElement = document.getElementById('app')!;
  appElement.innerHTML = `
    <div style="display: flex; align-items: center; justify-content: center; min-height: 100vh; padding: 2rem; background: linear-gradient(to bottom right, #eff6ff, #eef2ff);">
      <div style="max-width: 32rem; text-align: center;">
        <h1 style="font-size: 1.875rem; font-weight: bold; color: #1e40af; margin-bottom: 1rem;">Invalid Path</h1>
        <p style="color: #4b5563; margin-bottom: 1.5rem;">
          This path is reserved and cannot be used as a notebook.
        </p>
        <a href="/" style="display: inline-block; padding: 0.75rem 1.5rem; background: #2563eb; color: white; text-decoration: none; border-radius: 0.5rem; font-weight: 500;">
          Go to Main Notebook
        </a>
      </div>
    </div>
  `;
} else {
  // Valid notebook path - configure database and mount app
  const notebook = getCurrentNotebook();
  setDatabaseName(notebook.dbName);

  console.log(`[Jottery] Loading notebook: ${notebook.displayName} (${notebook.id})`);
  console.log(`[Jottery] Database: ${notebook.dbName}`);

  app = mount(App, {
    target: document.getElementById('app')!,
  });
}

export default app;
