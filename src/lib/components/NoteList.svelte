<script lang="ts">
  import { filteredNotes, notes, searchQuery, selectedNoteId } from '../stores/appStore';
  import NoteListItem from './NoteListItem.svelte';
  import ConfirmModal from './ConfirmModal.svelte';
  import { beforeUpdate, afterUpdate } from 'svelte';
  import { noteRepository } from '../services/noteRepository';
  import { keyManager } from '../services/keyManager';
  import type { DecryptedNote } from '../types';

  export let onNoteSelect: (() => void) | undefined = undefined;

  let scrollContainer: HTMLDivElement;
  let savedScrollTop = 0;
  let showDeleteConfirm = false;
  let showPinnedWarning = false;
  let noteToDelete: DecryptedNote | null = null;

  // Save scroll position before DOM updates
  beforeUpdate(() => {
    if (scrollContainer) {
      savedScrollTop = scrollContainer.scrollTop;
    }
  });

  // Restore scroll position after DOM updates
  afterUpdate(() => {
    if (scrollContainer && savedScrollTop >= 0) {
      scrollContainer.scrollTop = savedScrollTop;
    }
  });

  async function handleNoteDeleted(noteId: string) {
    // Reload notes after deletion
    const masterKey = keyManager.getMasterKey();
    if (masterKey) {
      await noteRepository.loadNotes(masterKey.key);
    }
  }

  function requestDelete(note: DecryptedNote) {
    // Check if note is pinned
    if (note.pinned) {
      noteToDelete = note;
      showPinnedWarning = true;
      return;
    }

    // Show confirmation modal
    noteToDelete = note;
    showDeleteConfirm = true;
  }

  async function confirmDelete() {
    if (!noteToDelete) return;

    showDeleteConfirm = false;

    try {
      const masterKey = keyManager.getMasterKey();
      if (!masterKey) {
        throw new Error('Application is locked');
      }

      await noteRepository.delete(noteToDelete.id);
      await handleNoteDeleted(noteToDelete.id);
      noteToDelete = null;
    } catch (error) {
      console.error('Failed to delete note:', error);
      noteToDelete = null;
    }
  }

  function cancelDelete() {
    showDeleteConfirm = false;
    noteToDelete = null;
  }

  function closePinnedWarning() {
    showPinnedWarning = false;
    noteToDelete = null;
  }

  function handleKeydown(event: KeyboardEvent) {
    // Handle Delete or Backspace key when a note is selected
    if ((event.key === 'Delete' || event.key === 'Backspace') && $selectedNoteId) {
      const selectedNote = $filteredNotes.find(n => n.id === $selectedNoteId);
      if (selectedNote) {
        event.preventDefault();
        requestDelete(selectedNote);
      }
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  bind:this={scrollContainer}
  class="h-full overflow-y-auto bg-white dark:bg-gray-900"
  on:keydown={handleKeydown}
  role="list"
  tabindex="-1"
>
  {#if $filteredNotes.length === 0}
    <div class="flex items-center justify-center h-full text-gray-500 dark:text-gray-400 p-4 text-center">
      <div>
        {#if $notes.length === 0}
          <p class="text-lg mb-2">No notes yet</p>
          <p class="text-sm">Create your first note to get started</p>
        {:else if $searchQuery.trim()}
          <p class="text-lg mb-2">No results found</p>
          <p class="text-sm">Try a different search query</p>
        {:else}
          <p class="text-lg mb-2">No notes</p>
          <p class="text-sm">Something went wrong</p>
        {/if}
      </div>
    </div>
  {:else}
    {#each $filteredNotes as note (note.id)}
      <NoteListItem {note} {onNoteSelect} onDeleteRequest={requestDelete} />
    {/each}
  {/if}
</div>

<!-- Delete confirmation modal -->
<ConfirmModal
  show={showDeleteConfirm}
  title="Delete Note"
  message={noteToDelete ? `Are you sure you want to delete "${noteToDelete.content.split('\n')[0] || 'Untitled'}"?\n\nThis will move the note to the recycle bin.` : ''}
  confirmText="Delete"
  cancelText="Cancel"
  onConfirm={confirmDelete}
  onCancel={cancelDelete}
/>

<!-- Pinned note warning modal -->
<ConfirmModal
  show={showPinnedWarning}
  title="Cannot Delete Pinned Note"
  message="This note is pinned. Please unpin it first before deleting."
  confirmText="OK"
  cancelText=""
  confirmClass="bg-blue-600 hover:bg-blue-700"
  onConfirm={closePinnedWarning}
  onCancel={closePinnedWarning}
/>
