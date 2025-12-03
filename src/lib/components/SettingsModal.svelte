<script lang="ts">
  import { settings, isLocked, notes } from '../stores/appStore';
  import { settingsRepository, deleteDB, noteService, searchService } from '../services';
  import { exportAllNotes, downloadExport, parseImportFile, importNotes } from '../services/exportService';
  import type { Theme } from '../types';
  import ConfirmModal from './ConfirmModal.svelte';

  export let show = false;
  export let onClose: () => void = () => {};
  export let onOpenShortcutsHelp: () => void = () => {};

  let theme: Theme = $settings.theme;
  let autoLockTimeout = $settings.autoLockTimeout;
  let sortOrder = $settings.sortOrder;
  let saving = false;
  let fileInput: HTMLInputElement;
  let showDeleteConfirm = false;

  // Apply theme when it changes
  $: applyTheme(theme);

  function applyTheme(selectedTheme: Theme) {
    if (typeof window === 'undefined') return;

    if (selectedTheme === 'dark') {
      document.documentElement.classList.add('dark');
    } else if (selectedTheme === 'light') {
      document.documentElement.classList.remove('dark');
    } else {
      // Auto mode - use system preference
      if (window.matchMedia('(prefers-color-scheme: dark)').matches) {
        document.documentElement.classList.add('dark');
      } else {
        document.documentElement.classList.remove('dark');
      }
    }
  }

  async function handleSave() {
    saving = true;
    try {
      await settingsRepository.update({
        theme,
        autoLockTimeout,
        sortOrder,
      });

      // Update store
      settings.update(s => ({
        ...s,
        theme,
        autoLockTimeout,
        sortOrder,
      }));

      onClose();
    } catch (error) {
      console.error('Failed to save settings:', error);
      alert('Failed to save settings: ' + (error instanceof Error ? error.message : String(error)));
    } finally {
      saving = false;
    }
  }

  function handleDeleteDatabase() {
    showDeleteConfirm = true;
  }

  async function confirmDeleteDatabase() {
    showDeleteConfirm = false;
    try {
      // Close database and delete
      await deleteDB();

      // Lock the app and reload
      isLocked.set(true);
      window.location.reload();
    } catch (error) {
      console.error('Failed to delete database:', error);
      alert('Failed to delete database: ' + (error instanceof Error ? error.message : String(error)));
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      onClose();
    }
  }

  async function handleExport() {
    try {
      const data = await exportAllNotes();
      await downloadExport(data);
    } catch (error) {
      console.error('Failed to export notes:', error);
      alert('Failed to export notes: ' + (error instanceof Error ? error.message : String(error)));
    }
  }

  async function handleImport() {
    fileInput.click();
  }

  async function handleFileSelect(event: Event) {
    const target = event.target as HTMLInputElement;
    const file = target.files?.[0];
    if (!file) return;

    try {
      const data = await parseImportFile(file);
      const result = await importNotes(data, 'merge');

      alert(`Import complete!\nImported: ${result.imported}\nSkipped: ${result.skipped}\n${result.errors.length > 0 ? 'Errors: ' + result.errors.join('\n') : ''}`);

      // Reload notes
      const allNotes = await noteService.getAllNotes($settings.sortOrder);
      notes.set(allNotes);
      searchService.indexNotes(allNotes);
    } catch (error) {
      console.error('Failed to import notes:', error);
      alert('Failed to import notes: ' + (error instanceof Error ? error.message : String(error)));
    } finally {
      // Clear file input
      target.value = '';
    }
  }
</script>

{#if show}
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
    on:click={handleBackdropClick}
    on:keydown={(e) => e.key === 'Escape' && onClose()}
    role="dialog"
    aria-modal="true"
  >
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-2xl w-full mx-4 max-h-[90vh] overflow-y-auto">
      <!-- Header -->
      <div class="border-b border-gray-200 dark:border-gray-700 p-4 flex items-center justify-between">
        <h2 class="text-xl font-bold text-gray-900 dark:text-white">Settings</h2>
        <button
          on:click={onClose}
          class="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
        >
          ✕
        </button>
      </div>

      <!-- Content -->
      <div class="p-6 space-y-6">
        <!-- Theme -->
        <div>
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            Theme
          </label>
          <select
            bind:value={theme}
            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
          >
            <option value="auto">Auto (System)</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
          <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">
            Choose your preferred color scheme
          </p>
        </div>

        <!-- Auto-lock timeout -->
        <div>
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            Auto-lock timeout (minutes)
          </label>
          <input
            type="number"
            bind:value={autoLockTimeout}
            min="1"
            max="1440"
            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
          <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">
            Application will lock after this period of inactivity
          </p>
        </div>

        <!-- Sort order -->
        <div>
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            Default sort order
          </label>
          <select
            bind:value={sortOrder}
            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
          >
            <option value="recent">Most recently modified</option>
            <option value="oldest">Oldest first</option>
            <option value="created">Recently created</option>
            <option value="alpha">Alphabetical</option>
          </select>
        </div>

        <!-- Import/Export -->
        <div class="border-t border-gray-200 dark:border-gray-700 pt-6">
          <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-4">Import/Export</h3>

          <div class="space-y-3">
            <div class="flex gap-2">
              <button
                on:click={handleExport}
                class="flex-1 px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-md transition-colors"
              >
                📤 Export All Notes
              </button>
              <button
                on:click={handleImport}
                class="flex-1 px-4 py-2 bg-green-600 hover:bg-green-700 text-white text-sm font-medium rounded-md transition-colors"
              >
                📥 Import Notes
              </button>
            </div>
            <p class="text-sm text-gray-500 dark:text-gray-400">
              Export notes as decrypted JSON. Import will merge notes with existing data.
            </p>
          </div>
        </div>

        <!-- Help & Resources -->
        <div class="border-t border-gray-200 dark:border-gray-700 pt-6">
          <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-4">Help & Resources</h3>

          <button
            on:click={onOpenShortcutsHelp}
            class="w-full px-4 py-2 bg-gray-100 hover:bg-gray-200 dark:bg-gray-700 dark:hover:bg-gray-600 text-gray-900 dark:text-white text-sm font-medium rounded-md transition-colors flex items-center justify-center gap-2"
          >
            ⌨️ Keyboard Shortcuts
          </button>
          <p class="mt-2 text-sm text-gray-500 dark:text-gray-400">
            View all available keyboard shortcuts
          </p>
        </div>

        <!-- Danger Zone -->
        <div class="border-t border-gray-200 dark:border-gray-700 pt-6">
          <h3 class="text-lg font-medium text-red-600 dark:text-red-400 mb-4">Danger Zone</h3>

          <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4">
            <h4 class="text-sm font-medium text-red-800 dark:text-red-200 mb-2">
              Delete All Data
            </h4>
            <p class="text-sm text-red-700 dark:text-red-300 mb-3">
              This will permanently delete ALL notes, settings, and encryption keys. This action cannot be undone.
            </p>
            <button
              on:click={handleDeleteDatabase}
              class="px-4 py-2 bg-red-600 hover:bg-red-700 text-white text-sm font-medium rounded-md transition-colors"
            >
              Delete Entire Database
            </button>
          </div>
        </div>
      </div>

      <!-- Footer -->
      <div class="border-t border-gray-200 dark:border-gray-700 p-4 flex justify-end gap-2">
        <button
          on:click={onClose}
          class="px-4 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-md transition-colors"
        >
          Cancel
        </button>
        <button
          on:click={handleSave}
          disabled={saving}
          class="px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white font-medium rounded-md transition-colors"
        >
          {saving ? 'Saving...' : 'Save Changes'}
        </button>
      </div>
    </div>

    <!-- Hidden file input for import -->
    <input
      bind:this={fileInput}
      type="file"
      accept=".json"
      on:change={handleFileSelect}
      class="hidden"
    />
  </div>

  <!-- Delete Database Confirmation Modal -->
  <ConfirmModal
    show={showDeleteConfirm}
    title="Delete All Data"
    message="This will permanently delete ALL notes, settings, and encryption keys. This action cannot be undone.{'\n\n'}Type DELETE to confirm:"
    confirmText="Delete Everything"
    cancelText="Cancel"
    confirmClass="bg-red-600 hover:bg-red-700"
    requireTextMatch="DELETE"
    onConfirm={confirmDeleteDatabase}
    onCancel={() => showDeleteConfirm = false}
  />
{/if}
