<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { selectedNoteIds, selectedCount, clearMultiSelection, notes, filteredNotes, selectAllFiltered } from '../stores/appStore';
  import { bulkOperationsService, type BulkProgress } from '../services/bulkOperationsService';
  import { toast } from '../utils/toast.svelte';
  import ConfirmModal from './ConfirmModal.svelte';
  import { get } from 'svelte/store';

  // Modal states
  let showAddTagsModal = false;
  let showRemoveTagsModal = false;
  let showCombineConfirm = false;
  let showDeleteConfirm = false;

  // Combine options
  let combineMergeTags = true;

  // Tag input
  let tagsInput = '';

  // Available tags for removal (collected from selected notes)
  let availableTags: string[] = [];
  let selectedTagsToRemove: Set<string> = new Set();

  // Progress state
  let progress: BulkProgress | null = null;
  let isProcessing = false;

  // Get all unique tags from selected notes
  function collectTagsFromSelected(): string[] {
    const allNotes = get(notes);
    const selectedIds = get(selectedNoteIds);
    const tagsSet = new Set<string>();

    for (const note of allNotes) {
      if (selectedIds.has(note.id)) {
        for (const tag of note.tags) {
          tagsSet.add(tag);
        }
      }
    }

    return Array.from(tagsSet).sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase()));
  }

  function openAddTagsModal() {
    tagsInput = '';
    showAddTagsModal = true;
  }

  function openRemoveTagsModal() {
    availableTags = collectTagsFromSelected();
    selectedTagsToRemove = new Set();
    showRemoveTagsModal = true;
  }

  function handleProgress(p: BulkProgress) {
    progress = p;
  }

  async function handleAddTags() {
    if (!tagsInput.trim()) return;

    const tags = tagsInput.split(',').map(t => t.trim()).filter(t => t.length > 0);
    if (tags.length === 0) return;

    const noteIds = Array.from(get(selectedNoteIds));
    isProcessing = true;
    showAddTagsModal = false;

    try {
      await bulkOperationsService.addTagsToNotes(noteIds, tags, handleProgress);
      toast.success($_('bulk.addTags') + `: ${tags.join(', ')}`);
      clearMultiSelection();
    } catch (error) {
      toast.error(String(error));
    } finally {
      isProcessing = false;
      progress = null;
    }
  }

  async function handleRemoveTags() {
    if (selectedTagsToRemove.size === 0) return;

    const tags = Array.from(selectedTagsToRemove);
    const noteIds = Array.from(get(selectedNoteIds));
    isProcessing = true;
    showRemoveTagsModal = false;

    try {
      await bulkOperationsService.removeTagsFromNotes(noteIds, tags, handleProgress);
      toast.success($_('bulk.removeTags') + `: ${tags.join(', ')}`);
      clearMultiSelection();
    } catch (error) {
      toast.error(String(error));
    } finally {
      isProcessing = false;
      progress = null;
    }
  }

  async function handleDelete() {
    const noteIds = Array.from(get(selectedNoteIds));
    isProcessing = true;
    showDeleteConfirm = false;

    try {
      await bulkOperationsService.deleteNotes(noteIds, handleProgress);
      toast.success($_('bulk.delete') + `: ${noteIds.length}`);
      // clearMultiSelection is called inside deleteNotes
    } catch (error) {
      toast.error(String(error));
    } finally {
      isProcessing = false;
      progress = null;
    }
  }

  async function handleExport() {
    const noteIds = Array.from(get(selectedNoteIds));
    isProcessing = true;

    try {
      const jsonData = await bulkOperationsService.exportNotes(noteIds, handleProgress);
      const timestamp = new Date().toISOString().slice(0, 10);
      bulkOperationsService.downloadExport(jsonData, `jottery-export-${timestamp}.json`);
      toast.success($_('bulk.export') + `: ${noteIds.length}`);
      clearMultiSelection();
    } catch (error) {
      toast.error(String(error));
    } finally {
      isProcessing = false;
      progress = null;
    }
  }

  function openCombineModal() {
    combineMergeTags = true; // Reset to default
    showCombineConfirm = true;
  }

  async function handleCombine() {
    const noteIds = Array.from(get(selectedNoteIds));
    isProcessing = true;
    showCombineConfirm = false;

    try {
      await bulkOperationsService.combineNotes(noteIds, handleProgress, { mergeTags: combineMergeTags });
      toast.success($_('bulk.combinedSuccess', { values: { count: noteIds.length } }));
      // clearMultiSelection and selectNote are called inside combineNotes
    } catch (error) {
      toast.error(String(error));
    } finally {
      isProcessing = false;
      progress = null;
    }
  }

  function toggleTagSelection(tag: string) {
    if (selectedTagsToRemove.has(tag)) {
      selectedTagsToRemove.delete(tag);
    } else {
      selectedTagsToRemove.add(tag);
    }
    selectedTagsToRemove = selectedTagsToRemove; // Trigger reactivity
  }

  function handleModalBackdrop(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      showAddTagsModal = false;
      showRemoveTagsModal = false;
    }
  }

  function handleModalKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      showAddTagsModal = false;
      showRemoveTagsModal = false;
    }
  }

  function handleSelectAll() {
    selectAllFiltered(get(filteredNotes));
  }

  // Check if all filtered notes are already selected
  $: allSelected = $filteredNotes.length > 0 && $filteredNotes.every(note => $selectedNoteIds.has(note.id));
</script>

{#if $selectedCount > 0}
  <!-- Fixed bottom toolbar -->
  <div class="fixed bottom-0 left-0 right-0 bg-white dark:bg-gray-800 border-t border-gray-200 dark:border-gray-700 shadow-lg z-40 safe-area-bottom">
    <div class="max-w-4xl mx-auto px-4 py-3">
      {#if isProcessing && progress}
        <!-- Progress bar -->
        <div class="flex items-center gap-4">
          <div class="flex-1">
            <div class="h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
              <div
                class="h-full bg-blue-600 transition-all duration-200"
                style="width: {(progress.current / progress.total) * 100}%"
              ></div>
            </div>
          </div>
          <span class="text-sm text-gray-600 dark:text-gray-400 whitespace-nowrap">
            {$_('bulk.progress', { values: { current: progress.current, total: progress.total } })}
          </span>
        </div>
      {:else}
        <!-- Toolbar buttons -->
        <div class="flex items-center justify-between gap-2 flex-wrap">
          <!-- Selection count and Select All -->
          <div class="flex items-center gap-3">
            <span class="text-sm font-medium text-gray-700 dark:text-gray-300">
              {$selectedCount} {$_('bulk.selected')}
            </span>
            {#if !allSelected}
              <button
                on:click={handleSelectAll}
                class="text-sm text-blue-600 dark:text-blue-400 hover:text-blue-800 dark:hover:text-blue-300 underline transition-colors"
              >
                {$_('bulk.selectAll')}
              </button>
            {/if}
          </div>

          <!-- Action buttons -->
          <div class="flex items-center gap-2 flex-wrap">
            <!-- Add Tags -->
            <button
              on:click={openAddTagsModal}
              class="px-3 py-1.5 text-sm bg-blue-600 text-white rounded-md hover:bg-blue-700 active:bg-blue-800 transition-colors"
            >
              {$_('bulk.addTags')}
            </button>

            <!-- Remove Tags -->
            <button
              on:click={openRemoveTagsModal}
              class="px-3 py-1.5 text-sm bg-gray-600 text-white rounded-md hover:bg-gray-700 active:bg-gray-800 transition-colors"
            >
              {$_('bulk.removeTags')}
            </button>

            <!-- Export -->
            <button
              on:click={handleExport}
              class="px-3 py-1.5 text-sm bg-green-600 text-white rounded-md hover:bg-green-700 active:bg-green-800 transition-colors"
            >
              {$_('bulk.export')}
            </button>

            <!-- Combine -->
            {#if $selectedCount >= 2}
              <button
                on:click={openCombineModal}
                class="px-3 py-1.5 text-sm bg-purple-600 text-white rounded-md hover:bg-purple-700 active:bg-purple-800 transition-colors"
              >
                {$_('bulk.combine')}
              </button>
            {/if}

            <!-- Delete -->
            <button
              on:click={() => showDeleteConfirm = true}
              class="px-3 py-1.5 text-sm bg-red-600 text-white rounded-md hover:bg-red-700 active:bg-red-800 transition-colors"
            >
              {$_('bulk.delete')}
            </button>

            <!-- Cancel -->
            <button
              on:click={clearMultiSelection}
              class="px-3 py-1.5 text-sm text-gray-600 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200 transition-colors"
            >
              {$_('bulk.cancel')}
            </button>
          </div>
        </div>
      {/if}
    </div>
  </div>
{/if}

<!-- Add Tags Modal -->
{#if showAddTagsModal}
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4"
    on:click={handleModalBackdrop}
    on:keydown={handleModalKeydown}
    role="dialog"
    aria-modal="true"
    tabindex="0"
  >
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full">
      <div class="border-b border-gray-200 dark:border-gray-700 p-4">
        <h2 class="text-xl font-bold text-gray-900 dark:text-white">{$_('bulk.addTags')}</h2>
      </div>

      <div class="p-6">
        <label for="tags-input" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
          {$_('bulk.enterTags')}
        </label>
        <input
          id="tags-input"
          type="text"
          bind:value={tagsInput}
          placeholder="tag1, tag2, tag3"
          class="w-full px-3 py-2.5 min-h-11 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
          on:keydown={(e) => e.key === 'Enter' && handleAddTags()}
        />
      </div>

      <div class="border-t border-gray-200 dark:border-gray-700 p-4 flex justify-end gap-3">
        <button
          on:click={() => showAddTagsModal = false}
          class="px-4 py-2.5 min-h-11 text-gray-700 dark:text-gray-300 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
        >
          {$_('common.cancel')}
        </button>
        <button
          on:click={handleAddTags}
          disabled={!tagsInput.trim()}
          class="px-4 py-2.5 min-h-11 bg-blue-600 text-white font-medium rounded-md active:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {$_('bulk.addTags')}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Remove Tags Modal -->
{#if showRemoveTagsModal}
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4"
    on:click={handleModalBackdrop}
    on:keydown={handleModalKeydown}
    role="dialog"
    aria-modal="true"
    tabindex="0"
  >
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full">
      <div class="border-b border-gray-200 dark:border-gray-700 p-4">
        <h2 class="text-xl font-bold text-gray-900 dark:text-white">{$_('bulk.removeTags')}</h2>
      </div>

      <div class="p-6">
        {#if availableTags.length === 0}
          <p class="text-gray-600 dark:text-gray-400">{$_('note.noTags')}</p>
        {:else}
          <p class="text-sm text-gray-600 dark:text-gray-400 mb-3">
            {$_('bulk.enterTags')}
          </p>
          <div class="flex flex-wrap gap-2 max-h-60 overflow-y-auto">
            {#each availableTags as tag}
              <button
                on:click={() => toggleTagSelection(tag)}
                class="px-3 py-1.5 rounded-full text-sm transition-colors {selectedTagsToRemove.has(tag)
                  ? 'bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200 ring-2 ring-red-500'
                  : 'bg-gray-100 dark:bg-gray-700 text-gray-800 dark:text-gray-200 hover:bg-gray-200 dark:hover:bg-gray-600'}"
              >
                #{tag}
              </button>
            {/each}
          </div>
        {/if}
      </div>

      <div class="border-t border-gray-200 dark:border-gray-700 p-4 flex justify-end gap-3">
        <button
          on:click={() => showRemoveTagsModal = false}
          class="px-4 py-2.5 min-h-11 text-gray-700 dark:text-gray-300 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
        >
          {$_('common.cancel')}
        </button>
        <button
          on:click={handleRemoveTags}
          disabled={selectedTagsToRemove.size === 0}
          class="px-4 py-2.5 min-h-11 bg-red-600 text-white font-medium rounded-md active:bg-red-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {$_('bulk.removeTags')} ({selectedTagsToRemove.size})
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Combine Confirmation Modal -->
{#if showCombineConfirm}
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4"
    on:click={(e) => e.target === e.currentTarget && (showCombineConfirm = false)}
    on:keydown={(e) => e.key === 'Escape' && (showCombineConfirm = false)}
    role="dialog"
    aria-modal="true"
    tabindex="0"
  >
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full">
      <!-- Header -->
      <div class="border-b border-gray-200 dark:border-gray-700 p-4">
        <h2 class="text-xl font-bold text-gray-900 dark:text-white">
          {$_('bulk.confirmCombine', { values: { count: $selectedCount } })}
        </h2>
      </div>

      <!-- Content -->
      <div class="p-6">
        <p class="text-gray-700 dark:text-gray-300 whitespace-pre-line mb-4">
          {$_('bulk.confirmCombineMessage', { values: { count: $selectedCount } })}
        </p>

        <!-- Merge tags checkbox -->
        <label class="flex items-center gap-3 cursor-pointer">
          <input
            type="checkbox"
            bind:checked={combineMergeTags}
            class="w-5 h-5 rounded border-gray-300 dark:border-gray-600 text-purple-600 focus:ring-purple-500"
          />
          <span class="text-gray-700 dark:text-gray-300">
            {$_('bulk.mergeTagsOption')}
          </span>
        </label>
        <p class="text-sm text-gray-500 dark:text-gray-400 mt-1 ml-8">
          {$_('bulk.mergeTagsDescription')}
        </p>
      </div>

      <!-- Footer -->
      <div class="border-t border-gray-200 dark:border-gray-700 p-4 flex justify-end gap-3">
        <button
          on:click={() => showCombineConfirm = false}
          class="px-4 py-2.5 min-h-11 text-gray-700 dark:text-gray-300 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
        >
          {$_('common.cancel')}
        </button>
        <button
          on:click={handleCombine}
          class="px-4 py-2.5 min-h-11 bg-purple-600 active:bg-purple-700 text-white font-medium rounded-md transition-colors"
        >
          {$_('bulk.combine')}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Delete Confirmation Modal -->
<ConfirmModal
  show={showDeleteConfirm}
  title={$_('bulk.confirmDelete', { values: { count: $selectedCount } })}
  message={$_('bulk.confirmDeleteMessage', { values: { count: $selectedCount } })}
  confirmText={$_('bulk.delete')}
  cancelText={$_('common.cancel')}
  confirmClass="bg-red-600 hover:bg-red-700"
  onConfirm={handleDelete}
  onCancel={() => showDeleteConfirm = false}
/>

<style>
  .safe-area-bottom {
    padding-bottom: env(safe-area-inset-bottom, 0);
  }
</style>
