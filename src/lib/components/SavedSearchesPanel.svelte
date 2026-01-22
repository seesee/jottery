<script lang="ts">
  import { onMount } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { modal, createBackdropHandler } from '../actions';
  import { savedSearchRepository } from '../services/savedSearchRepository';
  import type { SavedSearch } from '../types';
  import ConfirmModal from './ConfirmModal.svelte';

  export let show = false;
  export let currentQuery: string = '';
  export let onClose: () => void;
  export let onApply: (query: string) => void;

  let savedSearches: SavedSearch[] = [];
  let editingId: string | null = null;
  let editName: string = '';
  let editQuery: string = '';
  let showDeleteConfirm = false;
  let deleteTargetId: string | null = null;
  let deleteTargetName: string = '';
  let newSearchName: string = '';
  let showNewSearchForm = false;
  let draggedId: string | null = null;

  $: backdropHandler = createBackdropHandler(onClose);

  // Handle keyboard events on backdrop
  function handleBackdropKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      onClose();
    }
  }

  // Action to focus an element when it's rendered
  function focusOnMount(node: HTMLElement) {
    node.focus();
    return {};
  }

  onMount(async () => {
    await loadSavedSearches();
  });

  async function loadSavedSearches() {
    savedSearches = await savedSearchRepository.getAll();
  }

  function handleApply(query: string) {
    onApply(query);
    onClose();
  }

  function startEdit(search: SavedSearch) {
    editingId = search.id;
    editName = search.name;
    editQuery = search.query;
  }

  function cancelEdit() {
    editingId = null;
    editName = '';
    editQuery = '';
  }

  async function saveEdit() {
    if (editingId && editName.trim() && editQuery.trim()) {
      await savedSearchRepository.update(editingId, {
        name: editName.trim(),
        query: editQuery.trim(),
      });
      await loadSavedSearches();
      cancelEdit();
    }
  }

  function confirmDelete(search: SavedSearch) {
    deleteTargetId = search.id;
    deleteTargetName = search.name;
    showDeleteConfirm = true;
  }

  async function handleDelete() {
    if (deleteTargetId) {
      await savedSearchRepository.softDelete(deleteTargetId);
      await loadSavedSearches();
      deleteTargetId = null;
      deleteTargetName = '';
    }
    showDeleteConfirm = false;
  }

  function cancelDelete() {
    deleteTargetId = null;
    deleteTargetName = '';
    showDeleteConfirm = false;
  }

  function toggleNewSearchForm() {
    showNewSearchForm = !showNewSearchForm;
    if (showNewSearchForm) {
      newSearchName = '';
    }
  }

  async function createNewSearch() {
    if (newSearchName.trim() && currentQuery.trim()) {
      await savedSearchRepository.create(newSearchName.trim(), currentQuery.trim());
      await loadSavedSearches();
      newSearchName = '';
      showNewSearchForm = false;
    }
  }

  // Drag and drop handlers
  function handleDragStart(event: DragEvent, id: string) {
    if (event.dataTransfer) {
      draggedId = id;
      event.dataTransfer.effectAllowed = 'move';
      event.dataTransfer.setData('text/html', ''); // Required for Firefox
    }
  }

  function handleDragOver(event: DragEvent) {
    event.preventDefault();
    if (event.dataTransfer) {
      event.dataTransfer.dropEffect = 'move';
    }
  }

  function handleDrop(event: DragEvent, targetId: string) {
    event.preventDefault();
    if (draggedId && draggedId !== targetId) {
      reorderSearches(draggedId, targetId);
    }
    draggedId = null;
  }

  async function reorderSearches(movedId: string, targetId: string) {
    const movedIndex = savedSearches.findIndex((s) => s.id === movedId);
    const targetIndex = savedSearches.findIndex((s) => s.id === targetId);

    if (movedIndex !== -1 && targetIndex !== -1) {
      // Reorder locally
      const newOrder = [...savedSearches];
      const [movedItem] = newOrder.splice(movedIndex, 1);
      newOrder.splice(targetIndex, 0, movedItem);

      // Update order in repository
      const orderedIds = newOrder.map((s) => s.id);
      await savedSearchRepository.reorder(orderedIds);
      await loadSavedSearches();
    }
  }

  function handleKeydownEdit(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      event.preventDefault();
      saveEdit();
    } else if (event.key === 'Escape') {
      cancelEdit();
    }
  }

  function handleKeydownNew(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      event.preventDefault();
      createNewSearch();
    } else if (event.key === 'Escape') {
      toggleNewSearchForm();
    }
  }
</script>

{#if show}
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4"
    on:click={backdropHandler}
    on:keydown={handleBackdropKeydown}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    use:modal={{ onEscape: onClose }}
  >
    <div
      class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-2xl w-full max-h-[80vh] flex flex-col"
      role="document"
    >
      <!-- Header -->
      <div class="border-b border-gray-200 dark:border-gray-700 p-4">
        <h2 class="text-xl font-bold text-gray-900 dark:text-white">
          {$_('search.savedSearches')}
        </h2>
      </div>

      <!-- Content -->
      <div class="flex-1 overflow-y-auto p-4 space-y-2" role="list">
        {#if savedSearches.length === 0}
          <div class="text-center py-8 text-gray-500 dark:text-gray-400">
            <p>{$_('search.noSavedSearches')}</p>
          </div>
        {:else}
          {#each savedSearches as search (search.id)}
            <div
              draggable={editingId !== search.id}
              on:dragstart={(e) => handleDragStart(e, search.id)}
              on:dragover={handleDragOver}
              on:drop={(e) => handleDrop(e, search.id)}
              class="bg-gray-50 dark:bg-gray-700 rounded-lg p-3 border border-gray-200 dark:border-gray-600 {draggedId === search.id ? 'opacity-50' : ''} {editingId !== search.id ? 'cursor-move' : ''}"
              role="listitem"
            >
              {#if editingId === search.id}
                <!-- Edit mode -->
                <div class="space-y-2">
                  <input
                    type="text"
                    bind:value={editName}
                    on:keydown={handleKeydownEdit}
                    placeholder={$_('search.searchName')}
                    class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-800 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                    aria-label={$_('search.searchName')}
                  />
                  <textarea
                    bind:value={editQuery}
                    on:keydown={handleKeydownEdit}
                    placeholder={$_('search.query')}
                    rows="2"
                    class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-800 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500 font-mono text-sm resize-none"
                    aria-label={$_('search.query')}
                  ></textarea>
                  <div class="flex gap-2 justify-end">
                    <button
                      on:click={cancelEdit}
                      class="px-3 py-1.5 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-600 rounded-md transition-colors"
                    >
                      {$_('common.cancel')}
                    </button>
                    <button
                      on:click={saveEdit}
                      disabled={!editName.trim() || !editQuery.trim()}
                      class="px-3 py-1.5 text-sm bg-blue-500 hover:bg-blue-600 text-white rounded-md transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      {$_('search.saveSearch')}
                    </button>
                  </div>
                </div>
              {:else}
                <!-- View mode -->
                <div class="flex items-start gap-3">
                  <!-- Drag handle -->
                  <div class="text-gray-400 dark:text-gray-500 mt-1 cursor-move">
                    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 8h16M4 16h16" />
                    </svg>
                  </div>

                  <!-- Content -->
                  <div class="flex-1 min-w-0">
                    <h3 class="font-medium text-gray-900 dark:text-white truncate">
                      {search.name}
                    </h3>
                    <p class="text-sm text-gray-600 dark:text-gray-400 font-mono truncate mt-1">
                      {search.query}
                    </p>
                  </div>

                  <!-- Actions -->
                  <div class="flex gap-2 shrink-0">
                    <button
                      on:click={() => handleApply(search.query)}
                      class="px-3 py-1.5 text-sm bg-blue-500 hover:bg-blue-600 text-white rounded-md transition-colors"
                      title={$_('search.applySavedSearch')}
                    >
                      {$_('search.applySavedSearch')}
                    </button>
                    <button
                      on:click={() => startEdit(search)}
                      class="p-1.5 text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-600 rounded-md transition-colors"
                      title={$_('search.editSearch')}
                      aria-label={$_('search.editSearch')}
                    >
                      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                      </svg>
                    </button>
                    <button
                      on:click={() => confirmDelete(search)}
                      class="p-1.5 text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-md transition-colors"
                      title={$_('search.deleteSearch')}
                      aria-label={$_('search.deleteSearch')}
                    >
                      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                      </svg>
                    </button>
                  </div>
                </div>
              {/if}
            </div>
          {/each}
        {/if}

        <!-- New search form -->
        {#if showNewSearchForm}
          <div class="bg-blue-50 dark:bg-blue-900/20 rounded-lg p-3 border border-blue-200 dark:border-blue-700">
            <div class="space-y-2">
              <input
                type="text"
                bind:value={newSearchName}
                on:keydown={handleKeydownNew}
                placeholder={$_('search.searchName')}
                class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-800 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                aria-label={$_('search.searchName')}
                use:focusOnMount
              />
              <div class="bg-gray-100 dark:bg-gray-700 rounded-md px-3 py-2 font-mono text-sm text-gray-700 dark:text-gray-300">
                {currentQuery || $_('search.noCurrentQuery')}
              </div>
              <div class="flex gap-2 justify-end">
                <button
                  on:click={toggleNewSearchForm}
                  class="px-3 py-1.5 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-600 rounded-md transition-colors"
                >
                  {$_('common.cancel')}
                </button>
                <button
                  on:click={createNewSearch}
                  disabled={!newSearchName.trim() || !currentQuery.trim()}
                  class="px-3 py-1.5 text-sm bg-blue-500 hover:bg-blue-600 text-white rounded-md transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {$_('search.saveSearch')}
                </button>
              </div>
            </div>
          </div>
        {/if}
      </div>

      <!-- Footer -->
      <div class="border-t border-gray-200 dark:border-gray-700 p-4 flex justify-between gap-3">
        <button
          on:click={toggleNewSearchForm}
          disabled={showNewSearchForm || !currentQuery.trim()}
          class="px-4 py-2 bg-green-500 hover:bg-green-600 text-white rounded-md transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {$_('search.newSavedSearch')}
        </button>
        <button
          on:click={onClose}
          class="px-4 py-2 bg-gray-500 hover:bg-gray-600 text-white rounded-md transition-colors"
        >
          {$_('common.close')}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Delete confirmation modal -->
<ConfirmModal
  show={showDeleteConfirm}
  title={$_('search.deleteSearch')}
  message={$_('search.confirmDeleteSearch', { values: { name: deleteTargetName } })}
  confirmText={$_('common.delete')}
  cancelText={$_('common.cancel')}
  confirmClass="bg-red-600 hover:bg-red-700"
  onConfirm={handleDelete}
  onCancel={cancelDelete}
/>
