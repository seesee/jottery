<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { isLocked, filteredNotes, selectedNoteId, selectedNote, selectNote, clearSelection, notes, settings } from '../stores/appStore';
  import { lock, noteService, searchService } from '../services';

  export let onNewNote: () => void;
  export let onOpenSettings: () => void;
  export let onFocusSearch: () => void;

  function handleKeydown(event: KeyboardEvent) {
    // Don't trigger shortcuts when typing in inputs/textareas
    const target = event.target as HTMLElement;
    const isEditing = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable;

    const isMac = navigator.platform.toUpperCase().indexOf('MAC') >= 0;
    const modifier = isMac ? event.metaKey : event.ctrlKey;

    // Global shortcuts (work even when editing for some)
    if (modifier && event.key === 'k') {
      event.preventDefault();
      onFocusSearch();
      return;
    }

    if (modifier && event.key === 'n') {
      event.preventDefault();
      onNewNote();
      return;
    }

    if (modifier && event.key === 'l') {
      event.preventDefault();
      if (confirm('Lock the application? Unsaved changes will be lost.')) {
        lock();
        isLocked.set(true);
      }
      return;
    }

    if (modifier && event.key === ',') {
      event.preventDefault();
      onOpenSettings();
      return;
    }

    if (modifier && event.key === '/') {
      event.preventDefault();
      showShortcutsHelp();
      return;
    }

    // Copy note content
    if (modifier && event.shiftKey && (event.key === 'C' || event.key === 'c')) {
      event.preventDefault();
      handleCopyNote();
      return;
    }

    // Editor shortcuts
    if (event.key === 'Escape' && $selectedNoteId) {
      event.preventDefault();
      clearSelection();
      return;
    }

    // Don't process other shortcuts when editing
    if (isEditing) return;

    // Note list navigation
    if (event.key === 'ArrowDown' || event.key === 'j') {
      event.preventDefault();
      navigateNotes(1);
      return;
    }

    if (event.key === 'ArrowUp' || event.key === 'k') {
      event.preventDefault();
      navigateNotes(-1);
      return;
    }

    if (event.key === 'Enter' && !$selectedNoteId) {
      event.preventDefault();
      const firstNote = $filteredNotes[0];
      if (firstNote) {
        selectNote(firstNote.id);
      }
      return;
    }

    if (event.key === 'Delete' && $selectedNoteId) {
      event.preventDefault();
      handleDelete();
      return;
    }

    if (event.key === 'p' && $selectedNoteId) {
      event.preventDefault();
      handleTogglePin();
      return;
    }
  }

  function navigateNotes(direction: number) {
    if ($filteredNotes.length === 0) return;

    const currentIndex = $filteredNotes.findIndex(n => n.id === $selectedNoteId);
    let newIndex: number;

    if (currentIndex === -1) {
      newIndex = direction > 0 ? 0 : $filteredNotes.length - 1;
    } else {
      newIndex = currentIndex + direction;
      if (newIndex < 0) newIndex = 0;
      if (newIndex >= $filteredNotes.length) newIndex = $filteredNotes.length - 1;
    }

    selectNote($filteredNotes[newIndex].id);
  }

  async function handleDelete() {
    if (!$selectedNoteId) return;
    try {
      await noteService.deleteNote($selectedNoteId);
      clearSelection();

      // Reload all notes to refresh the UI
      const allNotes = await noteService.getAllNotes($settings.sortOrder);
      notes.set(allNotes);
      searchService.indexNotes(allNotes);
    } catch (error) {
      console.error('Failed to delete note:', error);
    }
  }

  async function handleTogglePin() {
    if (!$selectedNoteId) return;
    try {
      await noteService.togglePin($selectedNoteId);

      // Reload all notes to refresh the UI
      const allNotes = await noteService.getAllNotes($settings.sortOrder);
      notes.set(allNotes);
      searchService.indexNotes(allNotes);
    } catch (error) {
      console.error('Failed to toggle pin:', error);
    }
  }

  async function handleCopyNote() {
    if (!$selectedNote) return;

    try {
      await navigator.clipboard.writeText($selectedNote.content);
      // Could show a toast notification here
      console.log('Note content copied to clipboard');
    } catch (error) {
      console.error('Failed to copy note:', error);
      // Fallback for older browsers
      try {
        const textArea = document.createElement('textarea');
        textArea.value = $selectedNote.content;
        textArea.style.position = 'fixed';
        textArea.style.left = '-999999px';
        document.body.appendChild(textArea);
        textArea.select();
        document.execCommand('copy');
        document.body.removeChild(textArea);
        console.log('Note content copied to clipboard (fallback method)');
      } catch (fallbackError) {
        console.error('Failed to copy note (fallback):', fallbackError);
      }
    }
  }

  function showShortcutsHelp() {
    const shortcuts = `
Keyboard Shortcuts:

Global:
  Ctrl/Cmd + K - Focus search
  Ctrl/Cmd + N - New note
  Ctrl/Cmd + L - Lock application
  Ctrl/Cmd + , - Settings
  Ctrl/Cmd + / - Show this help

Note List:
  ↑/↓ or J/K - Navigate notes
  Enter - Open selected note
  Delete - Delete selected note
  P - Pin/unpin selected note

Editor:
  Esc - Close note
  Ctrl/Cmd + F - Find in note (CodeMirror)
  Ctrl/Cmd + H - Replace in note (CodeMirror)
    `;
    alert(shortcuts);
  }

  onMount(() => {
    window.addEventListener('keydown', handleKeydown);
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleKeydown);
  });
</script>
