<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { isLocked, isLocking, filteredNotes, selectedNoteId, selectedNote, selectNote, clearSelection, notes, settings } from '../stores/appStore';
  import { lock, noteService, searchService, syncService, syncRepository } from '../services';
  import { _ } from 'svelte-i18n';
  import type { KeyboardShortcut } from '../types';

  export let onNewNote: () => void;
  export let onOpenSettings: () => void;
  export let onFocusSearch: () => void;
  export let onOpenShortcutsHelp: () => void;

  async function handleLockNow() {
    isLocking.set(true);

    try {
      // Give any pending auto-saves time to complete (EditorPane has 1s debounce)
      await new Promise(resolve => setTimeout(resolve, 1200));

      // Trigger sync if enabled (will save all notes to server)
      if ($settings.syncEnabled) {
        const syncMetadata = await syncRepository.getMetadata();
        if (syncMetadata?.apiKey) {
          console.log('[KeyboardShortcuts] Syncing before lock...');
          await syncService.syncNow();
        }
      }
    } catch (error) {
      console.error('[KeyboardShortcuts] Error during pre-lock save/sync:', error);
      // Continue with lock even if sync fails
    }

    // Lock the application
    lock();
    isLocked.set(true);
    isLocking.set(false);
  }

  function matchesShortcut(event: KeyboardEvent, shortcut: KeyboardShortcut | undefined): boolean {
    // Null check - shortcut might be undefined
    if (!shortcut || !shortcut.key) return false;

    // On Mac, support both Cmd (metaKey) and Ctrl (ctrlKey) as "Ctrl"
    // On Windows/Linux, just use Ctrl (ctrlKey)
    const hasCtrl = event.metaKey || event.ctrlKey;
    const hasAlt = event.altKey;
    const hasShift = event.shiftKey;

    // For multi-modifier support, ALL modifiers must match exactly
    // Ctrl+N should NOT match Ctrl+Alt+N
    const ctrlMatches = (shortcut.ctrl === true) === hasCtrl;
    const altMatches = (shortcut.alt === true) === hasAlt;
    const shiftMatches = (shortcut.shift === true) === hasShift;

    if (!ctrlMatches || !altMatches || !shiftMatches) {
      return false;
    }

    // Check key matches (case insensitive)
    return event.key.toLowerCase() === shortcut.key.toLowerCase();
  }

  function handleKeydown(event: KeyboardEvent) {
    // Don't trigger shortcuts when typing in inputs/textareas
    const target = event.target as HTMLElement;
    const isEditing = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable;

    const shortcuts = $settings.keyboardShortcuts;
    if (!shortcuts) return;

    // Global shortcuts (work even when editing for some)
    if (matchesShortcut(event, shortcuts.focusSearch)) {
      event.preventDefault();
      onFocusSearch();
      return;
    }

    if (matchesShortcut(event, shortcuts.newNote)) {
      event.preventDefault();
      onNewNote();
      return;
    }

    if (matchesShortcut(event, shortcuts.lockApp)) {
      event.preventDefault();
      handleLockNow();
      return;
    }

    if (matchesShortcut(event, shortcuts.openSettings)) {
      event.preventDefault();
      onOpenSettings();
      return;
    }

    if (matchesShortcut(event, shortcuts.showShortcuts)) {
      event.preventDefault();
      onOpenShortcutsHelp();
      return;
    }

    // Copy note content
    if (matchesShortcut(event, shortcuts.copyNote)) {
      event.preventDefault();
      handleCopyNote();
      return;
    }

    // Editor shortcuts - only close note if no modals are open
    if (event.key === 'Escape' && $selectedNoteId) {
      // Check if any modal is open (they have z-50 class and are visible)
      const modals = document.querySelectorAll('.z-50');
      const hasOpenModal = Array.from(modals).some(modal => {
        const style = window.getComputedStyle(modal as HTMLElement);
        return style.display !== 'none' && style.visibility !== 'hidden';
      });

      // Only close the note if no modal is open
      if (!hasOpenModal) {
        event.preventDefault();
        clearSelection();
        return;
      }
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

      // Trigger background sync so pin state syncs to other devices
      if ($settings.syncEnabled) {
        syncService.syncNow();
      }
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

  onMount(() => {
    window.addEventListener('keydown', handleKeydown);
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleKeydown);
  });
</script>
