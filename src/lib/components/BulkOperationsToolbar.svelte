<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { onMount, onDestroy } from 'svelte';
  import { selectedNoteIds, selectedCount, clearMultiSelection, notes, filteredNotes, selectAllFiltered } from '../stores/appStore';
  import { bulkOperationsService, type BulkProgress } from '../services/bulkOperationsService';
  import { toast } from '../utils/toast.svelte';
  import ConfirmModal from './ConfirmModal.svelte';
  import ColorPickerModal from './ColorPickerModal.svelte';
  import { get } from 'svelte/store';
  import { modal } from '../actions';
  import { localeSort } from '../utils/stringUtils';

  // Modal states
  let showAddTagsModal = false;
  let showRemoveTagsModal = false;
  let showColorPicker = false;
  let showCombineConfirm = false;
  let showDeleteConfirm = false;

  // Overflow menu state (mobile)
  let showOverflow = false;
  let overflowContainerRef: HTMLDivElement;

  function handleOverflowClickOutside(event: MouseEvent) {
    if (showOverflow && overflowContainerRef && !overflowContainerRef.contains(event.target as Node)) {
      showOverflow = false;
    }
  }

  function handleOverflowKeydown(event: KeyboardEvent) {
    if (showOverflow && event.key === 'Escape') {
      showOverflow = false;
      event.preventDefault();
    }
  }

  function toggleOverflow() {
    showOverflow = !showOverflow;
  }

  // Overflow menu action wrappers (close menu then trigger action)
  function overflowRemoveTags() {
    showOverflow = false;
    openRemoveTagsModal();
  }

  function overflowSetColor() {
    showOverflow = false;
    showColorPicker = true;
  }

  function overflowExport() {
    showOverflow = false;
    handleExport();
  }

  function overflowCombine() {
    showOverflow = false;
    openCombineModal();
  }

  function overflowArchive() {
    showOverflow = false;
    handleArchive();
  }

  onMount(() => {
    document.addEventListener('click', handleOverflowClickOutside);
    document.addEventListener('keydown', handleOverflowKeydown);
  });

  onDestroy(() => {
    document.removeEventListener('click', handleOverflowClickOutside);
    document.removeEventListener('keydown', handleOverflowKeydown);
  });

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

    return Array.from(tagsSet).sort(localeSort);
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

  async function handleSetColor(color: string | undefined) {
    const noteIds = Array.from(get(selectedNoteIds));
    isProcessing = true;
    showColorPicker = false;

    try {
      await bulkOperationsService.setColorForNotes(noteIds, color, handleProgress);
      const colorName = color || $_('settings.colors.noColor');
      toast.success($_('bulk.colorSet', { values: { color: colorName, count: noteIds.length } }));
      clearMultiSelection();
    } catch (error) {
      toast.error(String(error));
    } finally {
      isProcessing = false;
      progress = null;
    }
  }

  async function handleArchive() {
    const noteIds = Array.from(get(selectedNoteIds));
    isProcessing = true;

    try {
      await bulkOperationsService.archiveNotes(noteIds, handleProgress);
      toast.success($_('bulk.archive') + `: ${noteIds.length}`);
      // clearMultiSelection is called inside archiveNotes
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

  function handleSelectAll() {
    selectAllFiltered(get(filteredNotes));
  }

  // Check if all filtered notes are already selected
  $: allSelected = $filteredNotes.length > 0 && $filteredNotes.every(note => $selectedNoteIds.has(note.id));
</script>

{#if $selectedCount > 0}
  <!-- Fixed bottom toolbar -->
  <div class="fixed bottom-0 left-0 right-0 bg-white dark:bg-gray-800 border-t border-gray-200 dark:border-gray-700 shadow-lg z-40 safe-area-bottom" data-testid="bulk-toolbar">
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

          <!-- Action buttons: Desktop (sm: and above) — full text labels -->
          <div class="hidden sm:flex items-center gap-2 flex-wrap">
            <!-- Add Tags -->
            <button
              on:click={openAddTagsModal}
              class="px-3 py-1.5 text-sm bg-blue-600 text-white rounded-md hover:bg-blue-700 active:bg-blue-800 transition-colors"
              data-testid="bulk-add-tags"
            >
              {$_('bulk.addTags')}
            </button>

            <!-- Remove Tags -->
            <button
              on:click={openRemoveTagsModal}
              class="px-3 py-1.5 text-sm bg-gray-600 text-white rounded-md hover:bg-gray-700 active:bg-gray-800 transition-colors"
              data-testid="bulk-remove-tags"
            >
              {$_('bulk.removeTags')}
            </button>

            <!-- Set Colour -->
            <button
              on:click={() => showColorPicker = true}
              class="px-3 py-1.5 text-sm bg-indigo-600 text-white rounded-md hover:bg-indigo-700 active:bg-indigo-800 transition-colors"
              data-testid="bulk-set-color"
            >
              {$_('bulk.setColor')}
            </button>

            <!-- Export -->
            <button
              on:click={handleExport}
              class="px-3 py-1.5 text-sm bg-green-600 text-white rounded-md hover:bg-green-700 active:bg-green-800 transition-colors"
              data-testid="bulk-export"
            >
              {$_('bulk.export')}
            </button>

            <!-- Combine -->
            {#if $selectedCount >= 2}
              <button
                on:click={openCombineModal}
                class="px-3 py-1.5 text-sm bg-purple-600 text-white rounded-md hover:bg-purple-700 active:bg-purple-800 transition-colors"
                data-testid="bulk-combine"
              >
                {$_('bulk.combine')}
              </button>
            {/if}

            <!-- Archive -->
            <button
              on:click={handleArchive}
              class="px-3 py-1.5 text-sm bg-amber-600 text-white rounded-md hover:bg-amber-700 active:bg-amber-800 transition-colors"
              data-testid="bulk-archive"
            >
              {$_('bulk.archive')}
            </button>

            <!-- Delete -->
            <button
              on:click={() => showDeleteConfirm = true}
              class="px-3 py-1.5 text-sm bg-red-600 text-white rounded-md hover:bg-red-700 active:bg-red-800 transition-colors"
              data-testid="bulk-delete"
            >
              {$_('bulk.delete')}
            </button>

            <!-- Cancel -->
            <button
              on:click={clearMultiSelection}
              class="px-3 py-1.5 text-sm text-gray-600 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200 transition-colors"
              data-testid="bulk-cancel"
            >
              {$_('bulk.cancel')}
            </button>
          </div>

          <!-- Action buttons: Mobile (below sm:) — icon-only with overflow menu -->
          <div class="flex sm:hidden items-center gap-1.5">
            <!-- Add Tags (icon) -->
            <button
              on:click={openAddTagsModal}
              class="p-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 active:bg-blue-800 transition-colors"
              title={$_('bulk.addTags')}
              data-testid="bulk-add-tags"
            >
              <!-- Heroicons: tag -->
              <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-5 h-5">
                <path stroke-linecap="round" stroke-linejoin="round" d="M9.568 3H5.25A2.25 2.25 0 0 0 3 5.25v4.318c0 .597.237 1.17.659 1.591l9.581 9.581c.699.699 1.78.872 2.607.33a18.095 18.095 0 0 0 5.223-5.223c.542-.827.369-1.908-.33-2.607L11.16 3.66A2.25 2.25 0 0 0 9.568 3Z" />
                <path stroke-linecap="round" stroke-linejoin="round" d="M6 6h.008v.008H6V6Z" />
              </svg>
            </button>

            <!-- Delete (icon) -->
            <button
              on:click={() => showDeleteConfirm = true}
              class="p-2 bg-red-600 text-white rounded-md hover:bg-red-700 active:bg-red-800 transition-colors"
              title={$_('bulk.delete')}
              data-testid="bulk-delete"
            >
              <!-- Heroicons: trash -->
              <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-5 h-5">
                <path stroke-linecap="round" stroke-linejoin="round" d="m14.74 9-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 0 1-2.244 2.077H8.084a2.25 2.25 0 0 1-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 0 0-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 0 1 3.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 0 0-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 0 0-7.5 0" />
              </svg>
            </button>

            <!-- Overflow menu -->
            <div class="relative" bind:this={overflowContainerRef}>
              <button
                on:click={toggleOverflow}
                class="p-2 text-gray-600 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-md transition-colors"
                title={$_('bulk.moreActions')}
              >
                <!-- Heroicons: ellipsis-horizontal -->
                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-5 h-5">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M6.75 12a.75.75 0 1 1-1.5 0 .75.75 0 0 1 1.5 0ZM12.75 12a.75.75 0 1 1-1.5 0 .75.75 0 0 1 1.5 0ZM18.75 12a.75.75 0 1 1-1.5 0 .75.75 0 0 1 1.5 0Z" />
                </svg>
              </button>

              {#if showOverflow}
                <div class="absolute bottom-full right-0 mb-2 w-48 bg-white dark:bg-gray-800 rounded-md shadow-lg border border-gray-200 dark:border-gray-700 z-50 overflow-hidden">
                  <div class="py-1">
                    <!-- Remove Tags -->
                    <button
                      on:click={overflowRemoveTags}
                      class="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
                      data-testid="bulk-remove-tags"
                    >
                      <!-- Heroicons: tag with minus indicator -->
                      <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-4 h-4 flex-shrink-0">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M9.568 3H5.25A2.25 2.25 0 0 0 3 5.25v4.318c0 .597.237 1.17.659 1.591l9.581 9.581c.699.699 1.78.872 2.607.33a18.095 18.095 0 0 0 5.223-5.223c.542-.827.369-1.908-.33-2.607L11.16 3.66A2.25 2.25 0 0 0 9.568 3Z" />
                        <path stroke-linecap="round" stroke-linejoin="round" d="M6 6h.008v.008H6V6Z" />
                        <path stroke-linecap="round" stroke-linejoin="round" d="M17.25 15h4.5" />
                      </svg>
                      {$_('bulk.removeTags')}
                    </button>

                    <!-- Set Colour -->
                    <button
                      on:click={overflowSetColor}
                      class="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
                      data-testid="bulk-set-color"
                    >
                      <!-- Heroicons: swatch -->
                      <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-4 h-4 flex-shrink-0">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M4.098 19.902a3.75 3.75 0 0 0 5.304 0l6.401-6.402M6.75 21A3.75 3.75 0 0 1 3 17.25V4.125C3 3.504 3.504 3 4.125 3h5.25c.621 0 1.125.504 1.125 1.125v4.072M6.75 21a3.75 3.75 0 0 0 3.75-3.75V8.197M6.75 21h13.125c.621 0 1.125-.504 1.125-1.125v-5.25c0-.621-.504-1.125-1.125-1.125h-4.072M10.5 8.197l2.88-2.88c.438-.439 1.15-.439 1.59 0l3.712 3.713c.44.44.44 1.152 0 1.59l-2.879 2.88M6.75 17.25h.008v.008H6.75v-.008Z" />
                      </svg>
                      {$_('bulk.setColor')}
                    </button>

                    <!-- Export -->
                    <button
                      on:click={overflowExport}
                      class="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
                      data-testid="bulk-export"
                    >
                      <!-- Heroicons: arrow-down-tray -->
                      <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-4 h-4 flex-shrink-0">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M3 16.5v2.25A2.25 2.25 0 0 0 5.25 21h13.5A2.25 2.25 0 0 0 21 18.75V16.5M16.5 12 12 16.5m0 0L7.5 12m4.5 4.5V3" />
                      </svg>
                      {$_('bulk.export')}
                    </button>

                    <!-- Combine (only when 2+ selected) -->
                    {#if $selectedCount >= 2}
                      <button
                        on:click={overflowCombine}
                        class="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
                        data-testid="bulk-combine"
                      >
                        <!-- Heroicons: document-duplicate -->
                        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-4 h-4 flex-shrink-0">
                          <path stroke-linecap="round" stroke-linejoin="round" d="M15.75 17.25v3.375c0 .621-.504 1.125-1.125 1.125h-9.75a1.125 1.125 0 0 1-1.125-1.125V7.875c0-.621.504-1.125 1.125-1.125H6.75a9.06 9.06 0 0 1 1.5.124m7.5 10.376h3.375c.621 0 1.125-.504 1.125-1.125V11.25c0-4.46-3.243-8.161-7.5-8.876a9.06 9.06 0 0 0-1.5-.124H9.375c-.621 0-1.125.504-1.125 1.125v3.5m7.5 10.375H9.375a1.125 1.125 0 0 1-1.125-1.125v-9.25m12 6.625v-1.875a3.375 3.375 0 0 0-3.375-3.375h-1.5a1.125 1.125 0 0 1-1.125-1.125v-1.5a3.375 3.375 0 0 0-3.375-3.375H9.75" />
                        </svg>
                        {$_('bulk.combine')}
                      </button>
                    {/if}

                    <!-- Archive -->
                    <button
                      on:click={overflowArchive}
                      class="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
                      data-testid="bulk-archive"
                    >
                      <!-- Heroicons: archive-box -->
                      <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-4 h-4 flex-shrink-0">
                        <path stroke-linecap="round" stroke-linejoin="round" d="m20.25 7.5-.625 10.632a2.25 2.25 0 0 1-2.247 2.118H6.622a2.25 2.25 0 0 1-2.247-2.118L3.75 7.5M10 11.25h4M3.375 7.5h17.25c.621 0 1.125-.504 1.125-1.125v-1.5c0-.621-.504-1.125-1.125-1.125H3.375c-.621 0-1.125.504-1.125 1.125v1.5c0 .621.504 1.125 1.125 1.125Z" />
                      </svg>
                      {$_('bulk.archive')}
                    </button>
                  </div>
                </div>
              {/if}
            </div>

            <!-- Cancel (icon) -->
            <button
              on:click={clearMultiSelection}
              class="p-2 text-gray-600 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-md transition-colors"
              title={$_('bulk.cancel')}
              data-testid="bulk-cancel"
            >
              <!-- Heroicons: x-mark -->
              <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-5 h-5">
                <path stroke-linecap="round" stroke-linejoin="round" d="M6 18 18 6M6 6l12 12" />
              </svg>
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
    on:keydown={(e) => e.key === 'Escape' && (showAddTagsModal = false)}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    use:modal={{ onEscape: () => showAddTagsModal = false }}
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
    on:keydown={(e) => e.key === 'Escape' && (showRemoveTagsModal = false)}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    use:modal={{ onEscape: () => showRemoveTagsModal = false }}
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
    tabindex="-1"
    use:modal={{ onEscape: () => showCombineConfirm = false }}
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

<!-- Color Picker Modal -->
<ColorPickerModal
  show={showColorPicker}
  currentColor={undefined}
  onColorSelect={handleSetColor}
  onClose={() => showColorPicker = false}
/>

<style>
  .safe-area-bottom {
    padding-bottom: env(safe-area-inset-bottom, 0);
  }
</style>
