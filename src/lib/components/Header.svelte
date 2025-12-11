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
  export let forceMobileLayout: boolean = false;

  let showLockConfirm = false;
  let showReleasesModal = false;
  let showMobileMenu = false;
  let showMobileSearch = false;

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

  function toggleMobileMenu() {
    showMobileMenu = !showMobileMenu;
  }

  function toggleMobileSearch() {
    showMobileSearch = !showMobileSearch;
    if (showMobileSearch) {
      // Focus search input after showing
      setTimeout(() => {
        document.getElementById('search-input-mobile')?.focus();
      }, 100);
    }
  }

  function closeMobileMenu() {
    showMobileMenu = false;
  }
</script>

<header class="border-b border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 px-4 py-3 relative">
  <div class="flex items-center gap-2 {forceMobileLayout ? 'gap-2' : 'tablet:gap-4'}">
    {#if forceMobileLayout}
      <!-- Mobile: Hamburger Menu Button -->
      <button
        on:click={toggleMobileMenu}
        class="min-h-11 min-w-11 p-2.5 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
        title="Menu"
        aria-label="Open menu"
      >
        <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
        </svg>
      </button>
    {:else}
      <button
        on:click={toggleMobileMenu}
        class="tablet:hidden min-h-11 min-w-11 p-2.5 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
        title="Menu"
        aria-label="Open menu"
      >
        <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
        </svg>
      </button>
    {/if}

    <!-- Brand -->
    <h1 class="text-lg {forceMobileLayout ? '' : 'tablet:text-xl'} font-bold text-gray-900 dark:text-white">{$_('app.name')}</h1>

    {#if !forceMobileLayout}
      <!-- Desktop: Search Bar -->
      <div class="hidden tablet:block flex-1 max-w-md relative">
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

      <div class="flex-1 tablet:hidden"></div>
    {:else}
      <div class="flex-1"></div>
    {/if}

    {#if forceMobileLayout}
      <!-- Mobile: Essential Actions -->
      <div class="flex items-center gap-3">
        <button
          on:click={toggleMobileSearch}
          class="min-h-11 min-w-11 p-3 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
          title={$_('search.placeholder')}
          aria-label="Search"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
        </button>

        <button
          on:click={handleNewNoteClick}
          class="min-h-11 min-w-11 p-3 bg-blue-600 active:bg-blue-700 text-white rounded-md transition-colors"
          title={$_('note.create')}
          aria-label="New note"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
          </svg>
        </button>
      </div>
    {:else}
      <!-- Mobile: Essential Actions (responsive) -->
      <div class="flex tablet:hidden items-center gap-3">
        <button
          on:click={toggleMobileSearch}
          class="min-h-11 min-w-11 p-3 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
          title={$_('search.placeholder')}
          aria-label="Search"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
        </button>

        <button
          on:click={handleNewNoteClick}
          class="min-h-11 min-w-11 p-3 bg-blue-600 active:bg-blue-700 text-white rounded-md transition-colors"
          title={$_('note.create')}
          aria-label="New note"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
          </svg>
        </button>
      </div>
    {/if}

    {#if !forceMobileLayout}
      <!-- Desktop: Full Actions -->
      <div class="hidden tablet:flex items-center gap-2">
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
    {/if}
  </div>

  <!-- Mobile: Expandable Search Bar -->
  {#if showMobileSearch}
    <div class="{forceMobileLayout ? '' : 'tablet:hidden'} mt-3 animate-slide-down">
      <div class="relative">
        <input
          id="search-input-mobile"
          type="text"
          bind:value={$searchQuery}
          placeholder={$_('search.placeholder')}
          class="w-full px-3 py-2 pr-8 border border-gray-300 dark:border-gray-600 rounded-md text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 dark:bg-gray-700 dark:text-white"
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
    </div>
  {/if}

  <!-- Mobile Menu Drawer -->
  {#if showMobileMenu}
    <!-- Backdrop -->
    <div
      class="{forceMobileLayout ? '' : 'tablet:hidden'} fixed inset-0 bg-black bg-opacity-50 z-40"
      on:click={closeMobileMenu}
      role="button"
      tabindex="-1"
      aria-label="Close menu"
    ></div>

    <!-- Drawer -->
    <div class="{forceMobileLayout ? '' : 'tablet:hidden'} fixed left-0 top-0 bottom-0 w-64 bg-white dark:bg-gray-800 z-50 shadow-xl animate-slide-in-left">
      <div class="flex flex-col h-full">
        <!-- Drawer Header -->
        <div class="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
          <h2 class="text-lg font-bold text-gray-900 dark:text-white">Menu</h2>
          <button
            on:click={closeMobileMenu}
            class="min-h-11 min-w-11 p-3 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
            aria-label="Close menu"
          >
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        <!-- Menu Items -->
        <nav class="flex-1 overflow-y-auto p-2">
          <button
            on:click={() => { onOpenRecycleBin(); closeMobileMenu(); }}
            class="w-full flex items-center gap-3 px-4 py-3 min-h-11 text-left active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
          >
            <span class="text-xl">🗑️</span>
            <span class="text-sm font-medium">{$_('recycleBin.title')}</span>
          </button>

          <button
            on:click={() => { onOpenSettings(); closeMobileMenu(); }}
            class="w-full flex items-center gap-3 px-4 py-3 min-h-11 text-left active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
          >
            <span class="text-xl">⚙️</span>
            <span class="text-sm font-medium">{$_('common.settings')}</span>
          </button>

          <button
            on:click={() => { handleOpenReleases(); closeMobileMenu(); }}
            class="w-full flex items-center gap-3 px-4 py-3 min-h-11 text-left active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
          >
            <span class="text-xl">🚀</span>
            <span class="text-sm font-medium">{$_('releases.title')}</span>
          </button>

          <div class="my-2 border-t border-gray-200 dark:border-gray-700"></div>

          <button
            on:click={() => { handleLockRequest(); closeMobileMenu(); }}
            class="w-full flex items-center gap-3 px-4 py-3 min-h-11 text-left active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
          >
            <span class="text-xl">🔒</span>
            <span class="text-sm font-medium">{$_('common.lock')}</span>
          </button>
        </nav>
      </div>
    </div>
  {/if}
</header>

<style>
  @keyframes slide-down {
    from {
      opacity: 0;
      transform: translateY(-10px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @keyframes slide-in-left {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(0);
    }
  }

  .animate-slide-down {
    animation: slide-down 0.2s ease-out;
  }

  .animate-slide-in-left {
    animation: slide-in-left 0.3s ease-out;
  }
</style>

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
