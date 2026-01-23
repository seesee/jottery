<script lang="ts">
  import { onMount, onDestroy, afterUpdate } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { selectedNote, clearSelection, notes, settings, isDraftMode, exitDraftMode, searchQuery, selectNote, isContentOnlyUpdate } from '../stores/appStore';
  import { noteService, tagService, searchService, attachmentService, syncService, versionRepository, noteRepository } from '../services';
  import { formatDateTime } from '../utils/dateFormat';
  import { formatShortcutForTooltip } from '../utils/keyboardShortcuts';
  import type { Attachment } from '../types';
  import VersionHistoryModal from './VersionHistoryModal.svelte';
  import AttachmentPreviewModal from './AttachmentPreviewModal.svelte';
  import { getPreviewHtml } from '../utils/markdownPreview';
  import { ALL_LANGUAGES } from '../utils/syntaxLanguages';
  import { toast } from '../utils/toast.svelte';
  import { resolveAttachmentPreviews } from '../utils/attachmentPreviewResolver';
  import { copyToClipboard, exportAsFile, printNote } from '../utils/noteExport';
  import { dropzone } from '../actions';
  import { EditorFooter, EditorToolbar, EditorContent, AttachmentsPanel, NoteInfoModal, MobileAttachmentsModal } from './editor';
  import ColorPickerModal from './ColorPickerModal.svelte';
  import { getColorHex, resolveTheme, hexWithOpacity } from '../services/colorService';

  export let onBackToList: (() => void) | undefined = undefined;
  export let forceMobileLayout: boolean = false;

  // Expose close method for external callers (e.g., Header back button)
  export function close() {
    handleClose();
  }

  let content = '';
  let tags: string[] = [];
  let attachments: Attachment[] = [];
  let isEditing = false;

  // Wrapper to handle closing note and returning to list on mobile
  // IMPORTANT: Navigate IMMEDIATELY for instant feedback, then save in background.
  // JavaScript promises continue to run even after component unmounts, so saves will complete.
  function handleClose() {
    // Exit draft mode if active
    if ($isDraftMode) {
      exitDraftMode();
    }

    // Capture values before navigating (component may unmount after navigation)
    const shouldSave = previousNoteId && hasContentChanged && !$isDraftMode;
    const noteIdToSave = previousNoteId;
    const contentToSave = content;
    const tagsToSave = [...tags];
    const attachmentsToSave = [...attachments];
    const languageToSave = language;
    const wordWrapToSave = wordWrap;
    const showPreviewToSave = showPreview;

    // Set content-only update flag so NoteList doesn't trigger full search
    // Only do this on mobile where we're navigating back to the list
    // On desktop, we want the full search to run so sorting is correct
    if (shouldSave && onBackToList) {
      isContentOnlyUpdate.set(true);
    }

    // Clear selection state
    clearSelection();

    // NAVIGATE IMMEDIATELY - this gives instant visual feedback
    if (onBackToList) {
      onBackToList();
    }

    // Fire off save operations in background (don't await)
    // These will complete even after component unmounts
    if (shouldSave && noteIdToSave) {
      (async () => {
        try {
          // Save current changes to IndexedDB
          await noteService.updateNote(noteIdToSave, {
            content: contentToSave,
            tags: tagsToSave,
            attachments: attachmentsToSave,
            syntaxLanguage: languageToSave,
            wordWrap: wordWrapToSave,
            showPreview: showPreviewToSave,
          });

          // Update the global notes store (store is global, so this works even after unmount)
          const updatedNote = await noteService.getNote(noteIdToSave);
          if (updatedNote) {
            notes.update(allNotes => {
              const index = allNotes.findIndex(n => n.id === updatedNote.id);
              if (index !== -1) {
                allNotes[index] = updatedNote;
              }
              return allNotes;
            });
          }

          // Create version snapshot
          await createVersionSnapshot(noteIdToSave);

          // Trigger sync
          triggerBackgroundSync();
        } catch (error) {
          console.error('[EditorPane] Error during background save:', error);
        }
      })();
    }
  }

  // Handle clicking on a tag to search for it
  function handleTagClick(tag: string) {
    searchQuery.set(`#${tag}`);
  }

  let saveTimeout: number | null = null;
  let language: string = 'plain';
  let wordWrap: boolean = true;
  let showPreview: boolean = false;
  let availableTags: string[] = [];
  let isUploading: boolean = false;
  let previousNoteId: string | null = null;
  let isDraggingFile: boolean = false;
  let isAttachmentsExpanded: boolean = false;
  let codeEditor: any = null; // Reference to CodeEditor component
  let showInfoModal: boolean = false;
  let showVersionHistory: boolean = false;
  let showAttachmentsModal: boolean = false; // Mobile: show attachments in modal
  let showColorPicker: boolean = false;
  let hasContentChanged: boolean = false; // Track if content modified since note loaded

  // Track blob URLs for cleanup
  let blobUrls: Set<string> = new Set();

  // Attachment preview state
  let previewAttachment: Attachment | null = null;

  // Lazy load highlight.js when preview is shown
  $: if (showPreview && !highlightJsLoaded && !loadingHighlightJs) {
    loadHighlightJs();
  }

  // Compute preview HTML
  $: previewHtml = showPreview ? getPreviewHtml(content, language, {
    attachments,
    openLinksInNewTab: $settings.openLinksInNewTab ?? true,
    hljs,
    loadingText: $_('editor.status.loading'),
  }) : '';

  // Check if preview is available for current language
  $: canPreview = language === 'markdown' || language === 'html' || language === 'xml';

  // Check if preview should use iframe (for HTML/XML with potential scripts)
  $: useIframePreview = language === 'html' || language === 'xml';

  // Compute note statistics
  $: noteStats = {
    characters: content.length,
    charactersNoSpaces: content.replace(/\s/g, '').length,
    words: content.trim().split(/\s+/).filter(s => s.length > 0).length,
    lines: content.split('\n').length,
    tags: tags.length,
    attachments: attachments.length,
  };

  // Format keyboard shortcuts for display
  $: shortcuts = $settings.keyboardShortcuts;
  $: copyNoteShortcut = formatShortcutForTooltip(shortcuts?.copyNote);
  $: noteInfoShortcut = formatShortcutForTooltip(shortcuts?.noteInfo);
  $: versionHistoryShortcut = formatShortcutForTooltip(shortcuts?.versionHistory);

  // Reactive date formatting stores
  $: createdAtFormatted = $selectedNote ? formatDateTime($selectedNote.createdAt) : null;
  $: modifiedAtFormatted = $selectedNote ? formatDateTime($selectedNote.modifiedAt) : null;

  // Lazy-loaded highlight.js for syntax highlighting
  let hljs: any = null;
  let loadingHighlightJs = false;
  let highlightJsLoaded = false;

  async function loadHighlightJs() {
    if (highlightJsLoaded || loadingHighlightJs) return;

    loadingHighlightJs = true;
    try {
      const syntaxHighlighter = await import('../utils/syntaxHighlighter');
      hljs = syntaxHighlighter.getHljsInstance();

      // Preload enabled syntax languages (filter out 'calc' which is a custom Jottery mode, not an hljs language)
      if ($settings.enabledSyntaxLanguages) {
        const hljsLanguages = $settings.enabledSyntaxLanguages.filter(lang => lang !== 'calc');
        await syntaxHighlighter.preloadLanguages(hljsLanguages);
      }

      highlightJsLoaded = true;
    } catch (error) {
      console.error('Failed to load syntax highlighter:', error);
    } finally {
      loadingHighlightJs = false;
    }
  }

  // Generate available language options (plain + enabled languages)
  $: availableLanguages = [
    { id: 'plain', name: $_('editor.plainText') },
    ...($settings.enabledSyntaxLanguages || [])
      .map(langId => {
        const lang = ALL_LANGUAGES.find(l => l.id === langId);
        return lang ? { id: lang.id, name: lang.name } : null;
      })
      .filter((lang): lang is { id: string; name: string } => lang !== null)
      .sort((a, b) => a.name.localeCompare(b.name))
  ];

  // Update available tags when notes change
  $: availableTags = tagService.getAllTags($notes);

  // Determine if dark mode is active
  $: isDark = $settings.theme === 'dark' ||
    ($settings.theme === 'auto' && window.matchMedia('(prefers-color-scheme: dark)').matches);

  // Compute note background color based on theme
  $: currentTheme = resolveTheme($settings.theme);
  $: noteBackgroundColor = $selectedNote?.color ? getColorHex($selectedNote.color, currentTheme) : undefined;

  // Watch for note selection changes
  $: if ($selectedNote) {
    // If in draft mode, ignore note selection changes
    if ($isDraftMode) {
      // Do nothing - draft mode takes priority
    } else {
      const noteChanged = previousNoteId !== $selectedNote.id;

      // Only reset local state when switching to a different note
      // Don't reset when the same note is reloaded (e.g., after save)
      if (noteChanged) {

      // Exit draft mode if we were in it
      if ($isDraftMode) {
        exitDraftMode();
      }

      // Save and create version for the previous note before switching
      if (previousNoteId && !$isDraftMode) {
        // Save immediately (don't wait for debounce)
        if (saveTimeout) {
          clearTimeout(saveTimeout);
          saveTimeout = null;
        }

        // Only save and create version if content was actually modified
        if (hasContentChanged) {
          // CRITICAL: Capture values in consts to prevent async closure bug
          // By the time the async function executes, these variables may have been reassigned
          const noteIdToSave = previousNoteId;
          const contentToSave = content;
          const tagsToSave = [...tags];
          const attachmentsToSave = [...attachments];
          const languageToSave = language;
          const wordWrapToSave = wordWrap;
          const showPreviewToSave = showPreview;

          // Perform save and version creation asynchronously
          (async () => {
            try {
              // Save current changes - use captured consts
              await noteService.updateNote(noteIdToSave, {
                content: contentToSave,
                tags: tagsToSave,
                attachments: attachmentsToSave,
                syntaxLanguage: languageToSave,
                wordWrap: wordWrapToSave,
                showPreview: showPreviewToSave,
              });

              // Update the store with saved data so switching back shows correct values
              const updatedNote = await noteService.getNote(noteIdToSave);
              if (updatedNote) {
                notes.update(allNotes => {
                  const index = allNotes.findIndex(n => n.id === updatedNote.id);
                  if (index !== -1) {
                    allNotes[index] = updatedNote;
                  }
                  return allNotes;
                });
              }

              // Create version snapshot
              await createVersionSnapshot(noteIdToSave);
            } catch (error) {
              console.error('[EditorPane] Error saving/versioning before switch:', error);
            }
          })();
        }
      } else if (saveTimeout) {
        // Just clear the timeout if in draft mode
        clearTimeout(saveTimeout);
        saveTimeout = null;
      }

      content = $selectedNote.content;
      tags = [...$selectedNote.tags];
      attachments = [...$selectedNote.attachments];
      language = $selectedNote.syntaxLanguage || 'plain';
      wordWrap = $selectedNote.wordWrap ?? true;
      showPreview = $selectedNote.showPreview ?? false;
      isEditing = true;
      hasContentChanged = false; // Reset change tracking for new note


      // Focus the editor after switching notes
      setTimeout(() => {
        if (codeEditor && !showPreview) {
          codeEditor.focus();
        }
      }, 10);

      // Trigger sync when switching notes (version snapshot will be created on close)
      if (previousNoteId) {
        triggerBackgroundSync();
      }
      } else {
        // Same note reloaded (from sync), not resetting state
      }

      previousNoteId = $selectedNote.id;
    }
  } else if (!$isDraftMode) {
    // Closing editor - flush pending save, create version, and trigger sync
    if (previousNoteId) {

      // Save immediately (don't wait for debounce)
      if (saveTimeout) {
        clearTimeout(saveTimeout);
        saveTimeout = null;
      }

      // Only save and create version if content was actually modified
      if (hasContentChanged) {
        // Capture previousNoteId before it gets reset
        const noteIdToSave = previousNoteId;

        // Perform the save, then create version
        (async () => {
          try {
            // Save current changes immediately
            await noteService.updateNote(noteIdToSave, {
              content,
              tags: tags,
              attachments: attachments,
              syntaxLanguage: language,
              wordWrap,
              showPreview,
            });

            // Update the store with saved data
            const updatedNote = await noteService.getNote(noteIdToSave);
            if (updatedNote) {
              notes.update(allNotes => {
                const index = allNotes.findIndex(n => n.id === updatedNote.id);
                if (index !== -1) {
                  allNotes[index] = updatedNote;
                }
                return allNotes;
              });
            }

            // Create version snapshot
            await createVersionSnapshot(noteIdToSave);

            await triggerBackgroundSync();
          } catch (error) {
            console.error('[EditorPane] Error during close save:', error);
          }
        })();
      } else {
        // Still trigger sync even if we didn't save
        triggerBackgroundSync();
      }
    }
    previousNoteId = null;

    content = '';
    tags = [];
    attachments = [];
    language = 'plain';
    wordWrap = true;
    showPreview = false;
    isEditing = false;
    hasContentChanged = false; // Reset change tracking
  }


  /**
   * Trigger background sync without blocking UI
   */
  function triggerBackgroundSync() {
    syncService.triggerBackgroundSync();
  }

  /**
   * Create a version snapshot for a specific note
   * Only called when navigating away or closing editor
   */
  async function createVersionSnapshot(noteId?: string) {
    // Use provided noteId or fall back to current selected note
    const targetNoteId = noteId || $selectedNote?.id;
    if (!targetNoteId) {
      return;
    }

    try {
      // Get the encrypted note from repository (not decrypted from service)
      const currentNote = await noteRepository.getById(targetNoteId);
      if (!currentNote) {
        return;
      }

      // Check for duplicate BEFORE incrementing version
      const latestVersion = await versionRepository.getLatestVersion(targetNoteId);
      if (latestVersion && latestVersion.content === currentNote.content) {
        return; // Skip if content unchanged
      }

      // Increment version only if we're actually creating a snapshot
      currentNote.version = (currentNote.version || 0) + 1;

      // Save the note with incremented version (update() will set modifiedAt)
      await noteRepository.update(currentNote);

      // Create version snapshot with the new version number
      await versionRepository.createVersion(currentNote, {
        syncedAt: new Date().toISOString(),
        reason: 'manual-sync',
      });
    } catch (error) {
      console.error('[EditorPane] Failed to create version for note:', targetNoteId, error);
    }
  }

  async function handleSave() {
    if (!$selectedNote) return;

    try {
      await noteService.updateNote($selectedNote.id, {
        content,
        tags: tags,
        attachments: attachments,
        syntaxLanguage: language,
        wordWrap,
        showPreview,
      });

      // Get just the updated note (much faster than reloading all notes)
      const updatedNote = await noteService.getNote($selectedNote.id);
      if (updatedNote) {
        // Update only this note in the store
        // This will trigger the reactive block in App.svelte which runs performSearch()
        // to maintain proper sort order based on modifiedAt
        notes.update(allNotes => {
          const index = allNotes.findIndex(n => n.id === updatedNote.id);
          if (index !== -1) {
            allNotes[index] = updatedNote;
          }
          return allNotes;
        });

        // Update only this note in search index (incremental update)
        searchService.updateNote(updatedNote);
      }

      // Trigger background sync after saving
      triggerBackgroundSync();

      // NOTE: Do NOT reset hasContentChanged here!
      // It should only be reset when:
      // 1. A new note is loaded (line 230)
      // 2. The editor is closed (line 312)
      // 3. After a version snapshot is created when switching notes
      // Resetting here after auto-save would prevent version creation
      // when the user later switches notes.

      // selectedNote will automatically update from the derived store
    } catch (error) {
      console.error('Failed to save note:', error);
    }
  }

  async function handleInput() {
    hasContentChanged = true;

    // Auto-save after 1 second of no typing
    if ($selectedNote) {
      if (saveTimeout) clearTimeout(saveTimeout);
      saveTimeout = window.setTimeout(handleSave, 1000);
    }
  }

  async function handleTogglePin() {
    if (!$selectedNote) return;
    try {
      await noteService.togglePin($selectedNote.id);

      // Get just the updated note (incremental update)
      const updatedNote = await noteService.getNote($selectedNote.id);
      if (updatedNote) {
        notes.update(allNotes => {
          const index = allNotes.findIndex(n => n.id === updatedNote.id);
          if (index !== -1) {
            allNotes[index] = updatedNote;
          }
          return allNotes;
        });
        searchService.updateNote(updatedNote);
      }

      // Trigger background sync so pin state syncs to other devices
      triggerBackgroundSync();

      // selectedNote will automatically update from the derived store
    } catch (error) {
      console.error('Failed to toggle pin:', error);
    }
  }

  async function handleArchive() {
    if (!$selectedNote) return;
    try {
      const noteId = $selectedNote.id;
      const isArchived = $selectedNote.archived;

      if (isArchived) {
        // Unarchive
        await noteService.unarchiveNote(noteId);
        toast.success($_('archive.unarchived'));
      } else {
        // Archive
        await noteService.archiveNote(noteId);
        handleClose(); // Close editor and return to list
      }

      // Get updated note
      const updatedNote = await noteService.getNote(noteId);
      if (updatedNote) {
        // Update the note in the store (don't remove it)
        notes.update(allNotes =>
          allNotes.map(n => n.id === noteId ? updatedNote : n)
        );
        searchService.updateNote(updatedNote);

        if (isArchived) {
          // Unarchived: select the note
          selectNote(updatedNote.id);
        }
      }

      // Trigger background sync
      triggerBackgroundSync();
    } catch (error) {
      console.error('Failed to toggle archive:', error);
      toast.error(`Failed to ${$selectedNote.archived ? 'unarchive' : 'archive'} note: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  async function handleDelete() {
    if (!$selectedNote) {
      return;
    }

    try {
      const noteId = $selectedNote.id;
      await noteService.deleteNote(noteId);
      handleClose(); // Clear selection and return to list on mobile

      // Remove note from store (incremental update)
      notes.update(allNotes => allNotes.filter(n => n.id !== noteId));

      // Remove from search index
      searchService.removeNote(noteId);

    } catch (error) {
      console.error('[EditorPane] Failed to delete note:', error);
    }
  }

  function handleLanguageChange(event: Event) {
    const target = event.target as HTMLSelectElement;
    const newLanguage = target.value as typeof language;

    // If we're in preview mode and switching to a language that doesn't support preview,
    // exit preview mode first
    const newLanguageSupportsPreview = newLanguage === 'markdown' || newLanguage === 'html';
    if (showPreview && !newLanguageSupportsPreview) {
      showPreview = false;
    }

    language = newLanguage;
    handleInput();
  }

  function handleWordWrapToggle() {
    wordWrap = !wordWrap;
    handleInput();
  }

  function handleUndo() {
    if (codeEditor) {
      codeEditor.undo();
    }
  }

  function handleRedo() {
    if (codeEditor) {
      codeEditor.redo();
    }
  }

  function handlePreviewToggle() {
    showPreview = !showPreview;
    // Save immediately for preview toggle (no debounce)
    if (saveTimeout) clearTimeout(saveTimeout);
    handleSave();
  }

  async function handleFileUpload(files: FileList) {
    if (!files || files.length === 0) return;

    isUploading = true;

    try {
      const newAttachments: Attachment[] = [];

      for (let i = 0; i < files.length; i++) {
        const file = files[i];

        // Validate file
        const validation = attachmentService.validateFile(file);
        if (!validation.valid) {
          toast.error(`${file.name}: ${validation.error}`);
          continue;
        }

        try {
          const attachment = await attachmentService.addAttachment(file);
          newAttachments.push(attachment);
        } catch (error) {
          console.error(`Failed to upload ${file.name}:`, error);
          toast.error(`Failed to upload ${file.name}: ${error instanceof Error ? error.message : String(error)}`);
        }
      }

      if (newAttachments.length > 0) {
        attachments = [...attachments, ...newAttachments];
        handleInput(); // Trigger auto-save
      }
    } finally {
      isUploading = false;
    }
  }

  async function handleDeleteAttachment(attachment: Attachment) {
    try {
      // Remove from attachments array
      attachments = attachments.filter(a => a.id !== attachment.id);

      // Delete from storage (this will happen when the note is saved)
      // We'll add cleanup in the noteService
      handleInput(); // Trigger auto-save
    } catch (error) {
      console.error('Failed to delete attachment:', error);
      toast.error(`Failed to delete attachment: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  // Show attachment preview
  function handlePreviewAttachment(attachment: Attachment) {
    previewAttachment = attachment;
  }

  // Close attachment preview
  function closeAttachmentPreview() {
    previewAttachment = null;
  }

  // Download attachment
  async function handleDownloadAttachment(attachment: Attachment) {
    try {
      await attachmentService.downloadAttachment(attachment);
    } catch (error) {
      console.error('Failed to download attachment:', error);
      toast.error(`Failed to download attachment: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  async function handleCopy() {
    await copyToClipboard(content);
  }

  function handleExport() {
    if (!$selectedNote) return;

    exportAsFile({
      content,
      language,
      createdAt: $selectedNote.createdAt,
    });
  }

  async function handlePrintPdf() {
    if (!$selectedNote) return;

    const success = await printNote({
      content,
      language,
      createdAt: $selectedNote.createdAt,
      previewHtml,
      attachments,
    });

    if (!success) {
      toast.error($_('editor.printFailed'));
    }
  }

  async function handleDuplicate() {
    if (!$selectedNote) return;

    try {
      // Create a duplicate of the current note
      const newNote = await noteService.duplicateNote($selectedNote.id);

      // Get the decrypted version
      const decryptedNote = await noteService.getNote(newNote.id);
      if (!decryptedNote) {
        throw new Error('Failed to retrieve duplicated note');
      }

      // Add to notes store
      notes.update(allNotes => {
        // Insert after pinned notes (new note is unpinned)
        const pinnedCount = allNotes.filter(n => n.pinned).length;
        const newNotes = [...allNotes];
        newNotes.splice(pinnedCount, 0, decryptedNote);
        return newNotes;
      });

      // Update search index
      searchService.updateNote(decryptedNote);

      // Select the new note
      selectNote(decryptedNote.id);

      // Trigger background sync
      triggerBackgroundSync();

      toast.success($_('editor.noteDuplicated'));
    } catch (error) {
      console.error('Failed to duplicate note:', error);
      toast.error($_('editor.duplicateFailed'));
    }
  }

  function handleShowInfo() {
    showInfoModal = true;
  }

  function handleShowVersionHistory() {
    showVersionHistory = true;
  }

  function handleShowAttachments() {
    showAttachmentsModal = true;
  }

  function handleSetColor() {
    showColorPicker = true;
  }

  async function handleColorSelected(color: string | undefined) {
    if (!$selectedNote || $isDraftMode) return;

    try {
      await noteService.updateNote($selectedNote.id, { color });

      // Update the global notes store
      const updatedNote = await noteService.getNote($selectedNote.id);
      if (updatedNote) {
        notes.update(allNotes => {
          const index = allNotes.findIndex(n => n.id === updatedNote.id);
          if (index !== -1) {
            allNotes[index] = updatedNote;
          }
          return allNotes;
        });

        // Update searchService index
        searchService.updateNote(updatedNote);

        // Also update selectedNote in the store
        selectNote($selectedNote.id);
      }

      // Trigger background sync
      triggerBackgroundSync();

      toast.info($_(color ? 'editor.colorSet' : 'editor.colorRemoved'));
    } catch (error) {
      console.error('[EditorPane] Error updating note color:', error);
      toast.error($_('editor.errors.updateFailed'));
    }
  }

  /**
   * Handle pasting an image from clipboard
   * Creates an attachment and returns a markdown image reference
   */
  async function handleImagePaste(file: File): Promise<string | null> {
    // Validate file
    const validation = attachmentService.validateFile(file);
    if (!validation.valid) {
      toast.error(validation.error || 'Invalid file');
      return null;
    }

    try {
      // Create the attachment
      const attachment = await attachmentService.addAttachment(file);

      // Add to attachments array
      attachments = [...attachments, attachment];

      // Trigger auto-save
      handleInput();

      // Generate alt text from filename or fallback
      const altText = file.name ? file.name.replace(/\.[^/.]+$/, '') : 'pasted image';

      // Return markdown image reference using attachment ID
      // Format: ![alt text](attachment:uuid)
      return `![${altText}](attachment:${attachment.id})`;
    } catch (error) {
      console.error('Failed to create attachment from pasted image:', error);
      toast.error(`Failed to paste image: ${error instanceof Error ? error.message : String(error)}`);
      return null;
    }
  }

  // Handle drag state change for file drop zone
  function handleDragStateChange(isDragging: boolean) {
    isDraggingFile = isDragging;
  }

  function toggleAttachments() {
    isAttachmentsExpanded = !isAttachmentsExpanded;
  }

  // Keyboard shortcut handler
  function matchesShortcut(event: KeyboardEvent, shortcut: any): boolean {
    if (!shortcut || !shortcut.key) return false;

    const hasCtrl = event.metaKey || event.ctrlKey;
    const hasAlt = event.altKey;
    const hasShift = event.shiftKey;

    const ctrlMatches = (shortcut.ctrl === true) === hasCtrl;
    const altMatches = (shortcut.alt === true) === hasAlt;
    const shiftMatches = (shortcut.shift === true) === hasShift;

    if (!ctrlMatches || !altMatches || !shiftMatches) {
      return false;
    }

    return event.key.toLowerCase() === shortcut.key.toLowerCase();
  }

  function handleEditorKeydown(event: KeyboardEvent) {
    // Only handle shortcuts when a note is selected
    if (!$selectedNote) return;

    const shortcuts = $settings.keyboardShortcuts;
    if (!shortcuts) return;

    // Undo
    if (matchesShortcut(event, shortcuts.undo)) {
      event.preventDefault();
      handleUndo();
      return;
    }

    // Redo
    if (matchesShortcut(event, shortcuts.redo)) {
      event.preventDefault();
      handleRedo();
      return;
    }

    // Version History
    if (matchesShortcut(event, shortcuts.versionHistory)) {
      event.preventDefault();
      handleShowVersionHistory();
      return;
    }

    // Note Info
    if (matchesShortcut(event, shortcuts.noteInfo)) {
      event.preventDefault();
      handleShowInfo();
      return;
    }
  }

  onMount(() => {
    window.addEventListener('keydown', handleEditorKeydown);
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleEditorKeydown);

    // Clean up all blob URLs
    blobUrls.forEach(url => URL.revokeObjectURL(url));
    blobUrls.clear();
  });

  // Process attachment URLs in preview mode
  afterUpdate(async () => {
    if (!showPreview) return;

    // Find the preview container
    const previewContainer = document.querySelector('.prose');
    if (!previewContainer) return;

    // Resolve all attachment references using the extracted utility
    await resolveAttachmentPreviews({
      container: previewContainer,
      attachments,
      blobUrls,
      onPreviewAttachment: handlePreviewAttachment,
      getErrorMessage: () => $_('editor.errors.loadingAttachment'),
    });
  });
</script>

{#if isEditing && ($selectedNote || $isDraftMode)}
  <div
    class="h-full flex flex-col bg-white dark:bg-gray-900"
    use:dropzone={{ onDrop: handleFileUpload, onDragStateChange: handleDragStateChange }}
    role="region"
    aria-label="Note editor"
  >
    <!-- Toolbar -->
    <EditorToolbar
      pinned={$selectedNote?.pinned || false}
      archived={$selectedNote?.archived || false}
      {language}
      {showPreview}
      {canPreview}
      attachmentCount={attachments.length}
      {forceMobileLayout}
      isDraftMode={$isDraftMode}
      {wordWrap}
      color={$selectedNote?.color}
      {availableLanguages}
      onPin={handleTogglePin}
      onArchive={handleArchive}
      onLanguageChange={handleLanguageChange}
      onTogglePreview={handlePreviewToggle}
      onShowAttachments={handleShowAttachments}
      onUndo={handleUndo}
      onRedo={handleRedo}
      onWordWrapToggle={handleWordWrapToggle}
      onSetColor={handleSetColor}
      onCopy={handleCopy}
      onExport={handleExport}
      onPrintPdf={handlePrintPdf}
      onDuplicate={handleDuplicate}
      onShowInfo={handleShowInfo}
      onShowVersionHistory={handleShowVersionHistory}
      onDelete={handleDelete}
      onClose={handleClose}
      {copyNoteShortcut}
      {noteInfoShortcut}
      {versionHistoryShortcut}
    />

    <!-- Tags Input + Content Editor/Preview -->
    <EditorContent
      {showPreview}
      {canPreview}
      bind:content
      bind:tags
      {language}
      {wordWrap}
      {isDark}
      {availableTags}
      bind:codeEditor
      onContentChange={() => handleInput()}
      onTagsChange={() => handleInput()}
      onTagClick={handleTagClick}
      onImagePaste={handleImagePaste}
      {previewHtml}
      {useIframePreview}
      rawContent={content}
    />

    <!-- Attachments Section - Only show if attachments exist or dragging files (hidden on mobile) -->
    {#if !forceMobileLayout && (attachments.length > 0 || isDraggingFile)}
      <AttachmentsPanel
        {attachments}
        isExpanded={isAttachmentsExpanded}
        {isDraggingFile}
        {isUploading}
        backgroundColor={noteBackgroundColor ? hexWithOpacity(noteBackgroundColor, 0.15) : undefined}
        onToggleExpanded={toggleAttachments}
        onDelete={handleDeleteAttachment}
        onFileUpload={handleFileUpload}
      />
    {/if}

    <!-- Metadata Footer -->
    {#if $selectedNote}
      <EditorFooter
        version={$selectedNote.version}
        {modifiedAtFormatted}
        backgroundColor={noteBackgroundColor ? hexWithOpacity(noteBackgroundColor, 0.3) : undefined}
      />
    {/if}
  </div>
{:else}
  <div class="h-full flex items-center justify-center text-gray-500 dark:text-gray-400">
    <div class="text-center">
      <p class="text-lg mb-2">{$_('editor.noNoteSelected')}</p>
      <p class="text-sm">{$_('editor.noNoteSelectedHint')}</p>
    </div>
  </div>
{/if}

<!-- Info Modal -->
{#if showInfoModal && $selectedNote}
  <NoteInfoModal
    show={showInfoModal}
    {noteStats}
    {language}
    noteId={$selectedNote.id}
    noteVersion={$selectedNote.version}
    {createdAtFormatted}
    {modifiedAtFormatted}
    onClose={() => showInfoModal = false}
  />
{/if}

<!-- Version History Modal -->
<VersionHistoryModal
  show={showVersionHistory}
  noteId={$selectedNote?.id}
  currentVersion={$selectedNote?.version}
  onClose={() => showVersionHistory = false}
  onRestore={async (versionNumber) => {
    if (!$selectedNote?.id) return;

    // Get the version to restore
    const version = await versionRepository.getVersion($selectedNote.id, versionNumber);
    if (!version) {
      console.error(`Version ${versionNumber} not found`);
      return;
    }

    // Get current note
    const currentNote = await noteRepository.getById($selectedNote.id);
    if (!currentNote) {
      console.error(`Note ${$selectedNote.id} not found`);
      return;
    }

    // Snapshot current state before restoring
    await versionRepository.createVersion(currentNote, {
      syncedAt: new Date().toISOString(),
      reason: 'manual-sync',
    });

    // Restore version data to the note in database
    currentNote.content = version.content;
    currentNote.tags = version.tags;
    currentNote.attachments = version.attachments;
    currentNote.syntaxLanguage = version.syntaxLanguage;
    currentNote.wordWrap = version.wordWrap;

    await noteRepository.update(currentNote);

    // Reload the note to get decrypted content for display
    const updatedNote = await noteService.getNote($selectedNote.id);
    if (updatedNote) {
      content = updatedNote.content;
      tags = updatedNote.tags;
      attachments = updatedNote.attachments;
      language = updatedNote.syntaxLanguage || 'plain';
      wordWrap = updatedNote.wordWrap ?? true;
      showPreview = updatedNote.showPreview ?? false;

      // Update note in store (incremental update)
      notes.update(allNotes => {
        const index = allNotes.findIndex(n => n.id === updatedNote.id);
        if (index !== -1) {
          allNotes[index] = updatedNote;
        }
        return allNotes;
      });
      searchService.updateNote(updatedNote);
    }
  }}
/>

<!-- Attachment Preview Modal -->
<AttachmentPreviewModal
  show={previewAttachment !== null}
  attachment={previewAttachment}
  onClose={closeAttachmentPreview}
  onDownload={handleDownloadAttachment}
/>

<!-- Attachments Modal (Mobile) -->
<MobileAttachmentsModal
  show={showAttachmentsModal}
  {forceMobileLayout}
  {attachments}
  {isUploading}
  onClose={() => showAttachmentsModal = false}
  onDelete={handleDeleteAttachment}
  onFileUpload={handleFileUpload}
/>

<!-- Color Picker Modal -->
<ColorPickerModal
  show={showColorPicker}
  currentColor={$selectedNote?.color}
  onColorSelect={handleColorSelected}
  onClose={() => showColorPicker = false}
/>
