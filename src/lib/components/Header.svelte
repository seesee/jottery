<script lang="ts">
  import { searchQuery, isLocked } from '../stores/appStore';
  import { lock } from '../services';
  import { _ } from 'svelte-i18n';
  import ConfirmModal from './ConfirmModal.svelte';
  import ReleasesModal from './ReleasesModal.svelte';

  export let onOpenSettings: () => void = () => {};
  export let onNewNote: () => void = () => {};
  export let onOpenRecycleBin: () => void = () => {};
  export let onOpenReleases: () => void = () => {};

  let showLockConfirm = false;
  let showReleasesModal = false;

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

  function handleOpenReleases() {
    showReleasesModal = true;
  }
</script>

<header class="border-b border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 px-4 py-3">
  <div class="flex items-center gap-4">
    <!-- Brand -->
    <h1 class="text-xl font-bold text-gray-900 dark:text-white">{$_('app.name')}</h1>

    <!-- Search Bar -->
    <div class="flex-1 max-w-md relative">
      <input
        id="search-input"
        type="text"
        bind:value={$searchQuery}
        placeholder={$_('search.placeholder')}
        class="w-full px-3 py-1.5 pr-8 border border-gray-300 dark:border-gray-600 rounded-md text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 dark:bg-gray-700 dark:text-white"
      />
      {#if $searchQuery}
        <button
          on:click={() => searchQuery.set('')}
          class="absolute right-2 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-200"
          title={$_('search.clear')}
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
        title={$_('note.create')}
      >
        + {$_('note.new')}
      </button>

      <button
        on:click={onOpenRecycleBin}
        class="px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700 text-sm rounded-md transition-colors"
        title={$_('recycleBin.title')}
      >
        🗑️ {$_('recycleBin.title')}
      </button>

      <button
        on:click={onOpenSettings}
        class="px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700 text-sm rounded-md transition-colors"
        title={$_('common.settings')}
      >
        ⚙️ {$_('common.settings')}
      </button>

      <button
        on:click={handleOpenReleases}
        class="px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700 text-sm rounded-md transition-colors"
        title={$_('releases.title')}
      >
        🚀 {$_('releases.title')}
      </button>

      <button
        on:click={handleLockRequest}
        class="px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700 text-sm rounded-md transition-colors"
        title={$_('keyboard.lockApp')}
      >
        🔒 {$_('common.lock')}
      </button>
    </div>
  </div>
</header>

<ConfirmModal
  show={showLockConfirm}
  title={$_('lock.title')}
  message={$_('lock.message')}
  confirmText={$_('lock.confirmButton')}
  cancelText={$_('common.cancel')}
  confirmClass="bg-blue-600 hover:bg-blue-700"
  onConfirm={handleLockConfirm}
  onCancel={handleLockCancel}
/>

<ReleasesModal
  show={showReleasesModal}
  onClose={() => showReleasesModal = false}
/>
