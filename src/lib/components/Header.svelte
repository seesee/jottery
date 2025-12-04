<script lang="ts">
  import { searchQuery, isLocked } from '../stores/appStore';
  import { lock } from '../services';
  import ConfirmModal from './ConfirmModal.svelte';

  export let onOpenSettings: () => void = () => {};
  export let onNewNote: () => void = () => {};
  export let onOpenRecycleBin: () => void = () => {};

  let showLockConfirm = false;

  function handleNewNoteClick() {
    // Call parent handler
    onNewNote();
  }

  function handleLockRequest() {
    showLockConfirm = true;
  }

  function handleLockConfirm() {
    showLockConfirm = false;
    lock();
    isLocked.set(true);
  }

  function handleLockCancel() {
    showLockConfirm = false;
  }
</script>

<header class="border-b border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 px-4 py-3">
  <div class="flex items-center gap-4">
    <!-- Brand -->
    <h1 class="text-xl font-bold text-gray-900 dark:text-white">Jottery</h1>

    <!-- Search Bar -->
    <div class="flex-1 max-w-md relative">
      <input
        id="search-input"
        type="text"
        bind:value={$searchQuery}
        placeholder="Search notes..."
        class="w-full px-3 py-1.5 pr-8 border border-gray-300 dark:border-gray-600 rounded-md text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 dark:bg-gray-700 dark:text-white"
      />
      {#if $searchQuery}
        <button
          on:click={() => searchQuery.set('')}
          class="absolute right-2 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-200"
          title="Clear search"
        >
          ✕
        </button>
      {/if}
    </div>

    <!-- Actions -->
    <div class="flex items-center gap-2">
      <button
        on:click={handleNewNoteClick}
        class="px-4 py-1.5 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-md transition-colors"
        title="Create new note (Ctrl+N)"
      >
        + New
      </button>

      <button
        on:click={onOpenRecycleBin}
        class="px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700 text-sm rounded-md transition-colors"
        title="Recycle Bin"
      >
        🗑️ Recycle Bin
      </button>

      <button
        on:click={onOpenSettings}
        class="px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700 text-sm rounded-md transition-colors"
        title="Settings"
      >
        ⚙️ Settings
      </button>

      <button
        on:click={handleLockRequest}
        class="px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700 text-sm rounded-md transition-colors"
        title="Lock application (Ctrl+L)"
      >
        🔒 Lock
      </button>
    </div>
  </div>
</header>

<ConfirmModal
  show={showLockConfirm}
  title="Lock Application"
  message="Lock the application? Unsaved changes will be lost."
  confirmText="Lock"
  cancelText="Cancel"
  confirmClass="bg-blue-600 hover:bg-blue-700"
  onConfirm={handleLockConfirm}
  onCancel={handleLockCancel}
/>
