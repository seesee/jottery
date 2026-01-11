<script lang="ts">
  import { onMount, onDestroy, afterUpdate } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { selectedNote, clearSelection, notes, settings, isDraftMode, exitDraftMode, searchQuery, isSyncRefreshing } from '../stores/appStore';
  import { noteService, tagService, searchService, attachmentService, syncService, syncRepository, versionRepository, noteRepository, keyManager, cryptoService } from '../services';
  import { formatDateTime } from '../utils/dateFormat';
  import { formatShortcutForTooltip } from '../utils/keyboardShortcuts';
  import type { Attachment } from '../types';
  import VersionHistoryModal from './VersionHistoryModal.svelte';
  import AttachmentPreviewModal from './AttachmentPreviewModal.svelte';
  import { getPreviewHtml } from '../utils/markdownPreview';
  import { ALL_LANGUAGES } from '../utils/syntaxLanguages';
  import { toast } from '../utils/toast.svelte';
  import { EditorFooter, EditorToolbar, EditorContent, AttachmentsPanel, NoteInfoModal, MobileAttachmentsModal } from './editor';

  export let onBackToList: (() => void) | undefined = undefined;
  export let forceMobileLayout: boolean = false;

  let content = '';
  let tags: string[] = [];
  let attachments: Attachment[] = [];
  let isEditing = false;

  // Wrapper to handle closing note and returning to list on mobile
  function handleClose() {
    // Exit draft mode if active
    if ($isDraftMode) {
      exitDraftMode();
    }
    clearSelection();
    if (onBackToList) {
      onBackToList();
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
  let dragCounter: number = 0; // Track nested drag events
  let codeEditor: any = null; // Reference to CodeEditor component
  let showInfoModal: boolean = false;
  let showVersionHistory: boolean = false;
  let showAttachmentsModal: boolean = false; // Mobile: show attachments in modal
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
  $: canPreview = language === 'markdown' || language === 'html';

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
  async function triggerBackgroundSync() {
    // Skip if sync is currently refreshing notes (prevents infinite loop)
    if ($isSyncRefreshing) {
      return;
    }
    try {
      const metadata = await syncRepository.getMetadata();
      if (metadata?.syncEnabled) {
        // Don't await - let it run in background
        syncService.syncNow().then(result => {
          if (!result.success) {
            console.warn('[EditorPane] Background sync failed:', result.error);
          }
        });
      }
    } catch (error) {
      console.error('[EditorPane] Failed to check sync status:', error);
    }
  }

  /**
   * Create a version snapshot for a specific note
   * Only called when navigating away or closing editor
   */
  async function createVersionSnapshot(noteId?: string) {
    // Use provided noteId or fall back to current selected note
    const targetNoteId = noteId || $selectedNote?.id;
    if (!targetNoteId) return;

    try {
      // Get the encrypted note from repository (not decrypted from service)
      const currentNote = await noteRepository.getById(targetNoteId);
      if (!currentNote) return;

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

      // Reset change tracking after successful save
      hasContentChanged = false;

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

      // selectedNote will automatically update from the derived store
    } catch (error) {
      console.error('Failed to toggle pin:', error);
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
    if (!content) return;

    try {
      await navigator.clipboard.writeText(content);
      // Could show a toast notification here
    } catch (error) {
      console.error('Failed to copy note:', error);
      // Fallback for older browsers
      try {
        const textArea = document.createElement('textarea');
        textArea.value = content;
        textArea.style.position = 'fixed';
        textArea.style.left = '-999999px';
        document.body.appendChild(textArea);
        textArea.select();
        document.execCommand('copy');
        document.body.removeChild(textArea);
      } catch (fallbackError) {
        console.error('Failed to copy note (fallback):', fallbackError);
      }
    }
  }

  function handleExport() {
    if (!content || !$selectedNote) return;

    // Map syntax language to file extension
    const extensionMap: Record<string, string> = {
      'plain': 'txt',
      'javascript': 'js',
      'python': 'py',
      'markdown': 'md',
      'json': 'json',
      'html': 'html',
      'css': 'css',
      'sql': 'sql',
      'bash': 'sh',
      'perl': 'pl',
    };

    const extension = extensionMap[language] || 'txt';

    // Generate filename from first line or date
    const firstLine = content.split('\n')[0].trim();
    const sanitizedFirstLine = firstLine
      .substring(0, 50) // Max 50 chars
      .replace(/[^a-z0-9_\-\.]/gi, '_') // Replace invalid chars
      .replace(/_{2,}/g, '_') // Replace multiple underscores
      .replace(/^_|_$/g, ''); // Trim underscores

    const filename = sanitizedFirstLine ||
      new Date($selectedNote?.createdAt || new Date()).toISOString().split('T')[0];

    // Create blob and download
    const blob = new Blob([content], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${filename}.${extension}`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);

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

  // Handle drag and drop for files
  function handleEditorDragEnter(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    dragCounter++;
    if (e.dataTransfer?.types.includes('Files')) {
      isDraggingFile = true;
    }
  }

  function handleEditorDragLeave(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    dragCounter--;
    if (dragCounter === 0) {
      isDraggingFile = false;
    }
  }

  function handleEditorDragOver(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
  }

  function handleEditorDrop(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    dragCounter = 0;
    isDraggingFile = false;

    const files = e.dataTransfer?.files;
    if (files && files.length > 0) {
      handleFileUpload(files);
    }
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

    // Find all attachment images and download links
    const previewContainer = document.querySelector('.prose');
    if (!previewContainer) return;

    const masterKey = keyManager.getMasterKey();
    if (!masterKey) return;

    // Process images with attachment IDs (already resolved)
    const imagesById = previewContainer.querySelectorAll('img[data-attachment-id]');
    for (const img of imagesById) {
      const attachmentId = img.getAttribute('data-attachment-id');
      if (!attachmentId) continue;

      // Skip if already loaded
      if (img.getAttribute('data-loaded') === 'true') continue;

      try {
        // Find the attachment object to get decrypted data
        const attachment = attachments.find(a => a.data === attachmentId);
        if (attachment) {
          // Load and decrypt the blob using attachmentService
          const blob = await attachmentService.getAttachmentData(attachment);
          const blobUrl = URL.createObjectURL(blob);
          img.setAttribute('src', blobUrl);
          img.setAttribute('data-loaded', 'true');

          // Track blob URL for cleanup
          blobUrls.add(blobUrl);
        } else {
          console.error(`Failed to find attachment with data: ${attachmentId}`);
          img.setAttribute('alt', '[Attachment not found]');
          img.setAttribute('data-loaded', 'true');
        }
      } catch (error) {
        console.error(`Failed to load attachment ${attachmentId}:`, error);
        img.setAttribute('alt', '[Failed to load image]');
        img.setAttribute('data-loaded', 'true'); // Mark as processed even on error
      }
    }

    // Process images with filenames (need to resolve)
    const imagesByFilename = previewContainer.querySelectorAll('img[data-attachment-filename]');
    for (const img of imagesByFilename) {
      const filename = img.getAttribute('data-attachment-filename');
      if (!filename) continue;

      // Skip if already loaded
      if (img.getAttribute('data-loaded') === 'true') continue;

      try {
        // Find attachment by decrypted filename
        let foundAttachment = null;

        for (const attachment of attachments) {
          try {
            const encryptedFilename = JSON.parse(attachment.filename);
            const decryptedFilename = await cryptoService.decryptText(encryptedFilename, masterKey.key);

            if (decryptedFilename === filename) {
              foundAttachment = attachment;
              break;
            }
          } catch (err) {
            console.error('Failed to decrypt filename for attachment:', attachment.id, err);
          }
        }

        if (foundAttachment) {
          // Load and decrypt the blob using attachmentService
          const blob = await attachmentService.getAttachmentData(foundAttachment);
          const blobUrl = URL.createObjectURL(blob);
          img.setAttribute('src', blobUrl);
          img.setAttribute('data-loaded', 'true');

          // Track blob URL for cleanup
          blobUrls.add(blobUrl);
        } else {
          console.error(`[Preview] No attachment found with filename: ${filename}`);
          img.setAttribute('alt', `[Attachment not found: ${filename}]`);
          img.setAttribute('data-loaded', 'true');
        }
      } catch (error) {
        console.error(`Failed to load attachment by filename ${filename}:`, error);
        img.setAttribute('alt', '[Failed to load image]');
        img.setAttribute('data-loaded', 'true');
      }
    }

    // Process download links
    const downloads = previewContainer.querySelectorAll('.attachment-download[data-attachment-id]');
    for (const div of downloads) {
      const attachmentId = div.getAttribute('data-attachment-id');
      if (!attachmentId) continue;

      // Skip if already processed
      if (div.classList.contains('clickable')) continue;

      const htmlDiv = div as HTMLElement;
      htmlDiv.classList.add('clickable');
      htmlDiv.style.cursor = 'pointer';

      div.addEventListener('click', async () => {
        try {
          const attachment = attachments.find(a => a.id === attachmentId);
          if (attachment) {
            // Open preview instead of downloading
            await handlePreviewAttachment(attachment);
          }
        } catch (error) {
          console.error(`Failed to preview attachment ${attachmentId}:`, error);
        }
      });
    }

    // Process download links by filename (need to resolve by decrypting filenames)
    const downloadsByFilename = previewContainer.querySelectorAll('.attachment-download[data-attachment-filename]');
    for (const div of downloadsByFilename) {
      const filename = div.getAttribute('data-attachment-filename');
      if (!filename) continue;

      // Skip if already loaded
      if (div.getAttribute('data-loaded') === 'true') continue;

      try {
        // Find attachment by decrypted filename
        let foundAttachment = null;

        for (const attachment of attachments) {
          try {
            const encryptedFilename = JSON.parse(attachment.filename);
            const decryptedFilename = await cryptoService.decryptText(encryptedFilename, masterKey.key);

            if (decryptedFilename === filename) {
              foundAttachment = attachment;
              break;
            }
          } catch (err) {
            console.error('Failed to decrypt filename for attachment:', attachment.id, err);
          }
        }

        if (foundAttachment) {
          // Update the div with the attachment ID and mark as loaded
          div.setAttribute('data-attachment-id', foundAttachment.id);
          div.setAttribute('data-attachment-data', foundAttachment.data);
          div.setAttribute('data-loaded', 'true');
          div.removeAttribute('data-attachment-filename');

          // Skip if already has click handler
          if (div.classList.contains('clickable')) continue;

          div.classList.add('clickable');

          div.addEventListener('click', async () => {
            try {
              await handlePreviewAttachment(foundAttachment);
            } catch (error) {
              console.error(`Failed to preview attachment ${foundAttachment.id}:`, error);
            }
          });
        } else {
          // Attachment not found - show error message
          console.error(`No attachment found with filename: ${filename}`);
          div.setAttribute('data-loaded', 'true');
          div.removeAttribute('data-attachment-filename');

          // Update content to show error
          div.innerHTML = `
            <span class="text-2xl mr-2">⚠️</span>
            <div class="flex-1">
              <span class="font-medium text-red-700 dark:text-red-400">Attachment not found</span>
              <span class="ml-2 text-sm text-gray-600 dark:text-gray-400">${filename}</span>
            </div>
          `;

          // Update styling to show error state
          div.classList.remove('bg-gray-50', 'dark:bg-gray-800', 'hover:bg-gray-100', 'dark:hover:bg-gray-700', 'cursor-pointer', 'border-gray-300', 'dark:border-gray-600');
          div.classList.add('bg-red-50', 'dark:bg-red-900/20', 'border-red-300', 'dark:border-red-700', 'cursor-not-allowed');
        }
      } catch (error) {
        console.error(`Failed to resolve attachment by filename ${filename}:`, error);
        div.setAttribute('data-loaded', 'true');
        div.removeAttribute('data-attachment-filename');

        // Show error state for resolution failure
        div.innerHTML = `
          <span class="text-2xl mr-2">⚠️</span>
          <div class="flex-1">
            <span class="font-medium text-red-700 dark:text-red-400">${$_('editor.errors.loadingAttachment')}</span>
            <span class="ml-2 text-sm text-gray-600 dark:text-gray-400">${filename}</span>
          </div>
        `;

        div.classList.remove('bg-gray-50', 'dark:bg-gray-800', 'hover:bg-gray-100', 'dark:hover:bg-gray-700', 'cursor-pointer', 'border-gray-300', 'dark:border-gray-600');
        div.classList.add('bg-red-50', 'dark:bg-red-900/20', 'border-red-300', 'dark:border-red-700', 'cursor-not-allowed');
      }
    }
  });
</script>

{#if isEditing && ($selectedNote || $isDraftMode)}
  <div
    class="h-full flex flex-col bg-white dark:bg-gray-900"
    on:dragenter={handleEditorDragEnter}
    on:dragleave={handleEditorDragLeave}
    on:dragover={handleEditorDragOver}
    on:drop={handleEditorDrop}
    role="region"
    aria-label="Note editor"
  >
    <!-- Toolbar -->
    <EditorToolbar
      pinned={$selectedNote?.pinned || false}
      {language}
      {showPreview}
      {canPreview}
      attachmentCount={attachments.length}
      {forceMobileLayout}
      isDraftMode={$isDraftMode}
      {wordWrap}
      {availableLanguages}
      onPin={handleTogglePin}
      onLanguageChange={handleLanguageChange}
      onTogglePreview={handlePreviewToggle}
      onShowAttachments={handleShowAttachments}
      onUndo={handleUndo}
      onRedo={handleRedo}
      onWordWrapToggle={handleWordWrapToggle}
      onCopy={handleCopy}
      onExport={handleExport}
      onShowInfo={handleShowInfo}
      onShowVersionHistory={handleShowVersionHistory}
      onDelete={handleDelete}
      onClose={handleClose}
      {onBackToList}
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
      {previewHtml}
    />

    <!-- Attachments Section - Only show if attachments exist or dragging files (hidden on mobile) -->
    {#if !forceMobileLayout && (attachments.length > 0 || isDraggingFile)}
      <AttachmentsPanel
        {attachments}
        isExpanded={isAttachmentsExpanded}
        {isDraggingFile}
        {isUploading}
        onToggleExpanded={toggleAttachments}
        onDelete={handleDeleteAttachment}
        onFileUpload={handleFileUpload}
      />
    {/if}

    <!-- Metadata Footer -->
    {#if $selectedNote}
      <EditorFooter {createdAtFormatted} {modifiedAtFormatted} />
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
