<script lang="ts">
  import { onMount, onDestroy, afterUpdate } from 'svelte';
  import { get } from 'svelte/store';
  import { _ } from 'svelte-i18n';
  import { selectedNote, clearSelection, notes, settings, isDraftMode, exitDraftMode, selectNote, searchQuery } from '../stores/appStore';
  import { noteService, tagService, searchService, attachmentService, syncService, syncRepository, versionRepository, noteRepository, attachmentRepository, keyManager, cryptoService } from '../services';
  import { formatDateTime } from '../utils/dateFormat';
  import { formatShortcutForTooltip } from '../utils/keyboardShortcuts';
  import type { Attachment } from '../types';
  import CodeEditor from './CodeEditor.svelte';
  import TagInput from './TagInput.svelte';
  import AttachmentList from './AttachmentList.svelte';
  import FileUpload from './FileUpload.svelte';
  import VersionHistoryModal from './VersionHistoryModal.svelte';
  import PdfViewer from './PdfViewer.svelte';
  import { marked } from 'marked';
  import { ALL_LANGUAGES, findLanguage } from '../utils/syntaxLanguages';
  import { toast } from '../utils/toast.svelte';
  import { EditorFooter, EditorToolbar, EditorContent } from './editor';

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
  let draftCreateTimeout: number | null = null; // Debounce timer for creating draft notes
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
  let previewContent: string | null = null;
  let previewType: 'image' | 'text' | 'pdf' | 'audio' | 'video' | 'unsupported' | null = null;
  let isLoadingPreview = false;

  // Lazy load highlight.js when preview is shown
  $: if (showPreview && !highlightJsLoaded && !loadingHighlightJs) {
    loadHighlightJs();
  }

  // Compute preview HTML
  $: previewHtml = showPreview ? getPreviewHtml(content, language) : '';

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

      // Preload enabled syntax languages
      if ($settings.enabledSyntaxLanguages) {
        await syntaxHighlighter.preloadLanguages($settings.enabledSyntaxLanguages);
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

  function getPreviewHtml(text: string, lang: string): string {
    if (lang === 'markdown') {
      try {
        // Configure marked for better rendering with syntax highlighting
        marked.setOptions({
          breaks: true, // Convert \n to <br>
          gfm: true, // GitHub Flavored Markdown
          headerIds: true,
          mangle: false,
          sanitize: false,
        });

        // Set up custom renderer for code blocks and images
        const renderer = new marked.Renderer();
        const originalCode = renderer.code.bind(renderer);
        const originalImage = renderer.image.bind(renderer);
        const originalLink = renderer.link.bind(renderer);

        // Custom link renderer to handle attachment: URLs
        renderer.link = function(token) {
          const href = token.href;
          const title = token.title || '';
          const text = token.text || '';

          // Check if this is an attachment URL
          if (href && href.startsWith('attachment:')) {
            const attachmentRef = href.substring('attachment:'.length);

            // Find the attachment
            let attachment = attachments.find(a => {
              if (a.id === attachmentRef) return true;
              const idWithoutHyphens = a.id.replace(/-/g, '');
              if (attachmentRef.startsWith(idWithoutHyphens)) return true;
              if (a.data === attachmentRef) return true;
              const dataWithoutHyphens = a.data.replace(/-/g, '');
              if (attachmentRef.startsWith(dataWithoutHyphens)) return true;
              return false;
            });

            if (attachment) {
              // Create a clickable div with attachment ID
              const icon = attachment.mimeType.includes('pdf') ? '📄' :
                          attachment.mimeType.startsWith('image/') ? '🖼️' :
                          attachment.mimeType.startsWith('video/') ? '🎥' :
                          attachment.mimeType.startsWith('audio/') ? '🎵' : '📎';
              return `<div class="attachment-download p-3 my-2 border border-gray-300 dark:border-gray-600 rounded bg-gray-50 dark:bg-gray-800 cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors" data-attachment-id="${attachment.id}" data-attachment-data="${attachment.data}">
                <span class="text-2xl mr-2">${icon}</span>
                <span class="font-medium text-gray-900 dark:text-gray-100">${text || attachmentRef}</span>
                <span class="ml-2 text-gray-600 dark:text-gray-400 text-sm">(${(attachment.size / 1024).toFixed(1)} KB)</span>
              </div>`;
            } else {
              // Attachment not found by ID - create placeholder with filename for async resolution
              return `<div class="attachment-download p-3 my-2 border border-gray-300 dark:border-gray-600 rounded bg-gray-50 dark:bg-gray-800 cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors" data-attachment-filename="${attachmentRef}" data-loaded="false">
                <span class="text-2xl mr-2">📎</span>
                <span class="font-medium text-gray-900 dark:text-gray-100">${text || attachmentRef}</span>
              </div>`;
            }
          }

          // For non-attachment links, use default renderer
          return originalLink.call(this, token);
        };

        // Custom image renderer to handle attachment: URLs
        renderer.image = function(token) {
          const href = token.href;
          const title = token.title || '';
          const text = token.text || '';

          // Check if this is an attachment URL
          if (href && href.startsWith('attachment:')) {
            const attachmentRef = href.substring('attachment:'.length);

            // Find the attachment by filename (preferred) or UUID
            // Note: We can't decrypt filenames synchronously in the renderer,
            // so we need to handle this in afterUpdate. For now, try to match by UUID.
            let attachment = attachments.find(a => {
              // Direct ID match (UUID)
              if (a.id === attachmentRef) return true;

              // Match UUID without hyphens
              const idWithoutHyphens = a.id.replace(/-/g, '');
              if (attachmentRef.startsWith(idWithoutHyphens)) return true;

              // Match by data field
              if (a.data === attachmentRef) return true;

              // Match data without hyphens
              const dataWithoutHyphens = a.data.replace(/-/g, '');
              if (attachmentRef.startsWith(dataWithoutHyphens)) return true;

              return false;
            });

            // If not found by UUID, mark for filename lookup in afterUpdate
            // Store the reference in the data attribute for async resolution
            if (!attachment) {
              // Return placeholder that will be resolved by filename in afterUpdate
              return `<img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='100' height='100'%3E%3Ctext x='50%25' y='50%25' text-anchor='middle' fill='%23999'%3E${get(_)('editor.status.loading')}%3C/text%3E%3C/svg%3E" data-attachment-filename="${attachmentRef}" data-loaded="false" alt="${text}" title="${title}" class="attachment-image" style="max-width: 100%; height: auto;" />`;
            }

            if (attachment) {
              // Check if it's an image
              if (attachment.mimeType.startsWith('image/')) {
                // Return a placeholder with data URL to avoid CSP errors
                // Store the actual attachment.data value (blob storage key) in data attribute
                return `<img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='100' height='100'%3E%3Ctext x='50%25' y='50%25' text-anchor='middle' fill='%23999'%3E${get(_)('editor.status.loading')}%3C/text%3E%3C/svg%3E" data-attachment-id="${attachment.data}" data-loaded="false" alt="${text}" title="${title}" class="attachment-image" style="max-width: 100%; height: auto;" />`;
              } else {
                // For non-image attachments, create a download link
                const icon = attachment.mimeType.includes('pdf') ? '📄' :
                            attachment.mimeType.startsWith('video/') ? '🎥' :
                            attachment.mimeType.startsWith('audio/') ? '🎵' : '📎';
                return `<div class="attachment-download p-3 my-2 border border-gray-300 dark:border-gray-600 rounded bg-gray-50 dark:bg-gray-800 cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors" data-attachment-id="${attachment.id}" data-attachment-data="${attachment.data}">
                  <span class="text-2xl mr-2">${icon}</span>
                  <span class="font-medium text-gray-900 dark:text-gray-100">${text || attachmentRef}</span>
                  <span class="ml-2 text-gray-600 dark:text-gray-400 text-sm">(${(attachment.size / 1024).toFixed(1)} KB)</span>
                </div>`;
              }
            }
          }

          // For non-attachment images, use default renderer
          return originalImage.call(this, token);
        };

        renderer.code = function(token) {
          // In marked v17+, renderer receives a token object instead of separate parameters
          const code = token.text;
          const language = token.lang;

          console.log('[Preview] Code block:', {
            language,
            codeLength: code?.length,
            hasLanguage: !!language,
            hljsLoaded: !!hljs
          });

          // Only use syntax highlighting if hljs is loaded
          if (hljs) {
            // Highlight the code if language is specified
            if (language && hljs.getLanguage(language)) {
              try {
                console.log('[Preview] Highlighting with language:', language);
                const highlighted = hljs.highlight(code, { language }).value;
                return `<pre><code class="hljs language-${language}">${highlighted}</code></pre>`;
              } catch (err) {
                console.error('Syntax highlighting error:', err);
              }
            }

            // Auto-detect if no language specified
            if (code) {
              try {
                console.log('[Preview] Auto-detecting language for code block');
                const highlighted = hljs.highlightAuto(code).value;
                return `<pre><code class="hljs">${highlighted}</code></pre>`;
              } catch (err) {
                console.error('Auto-highlight error:', err);
              }
            }
          }

          // Fallback to plain code block if hljs not loaded or highlighting failed
          console.log('[Preview] Falling back to default rendering');
          return originalCode.call(this, token);
        };

        return marked.parse(text, { renderer });
      } catch (error) {
        console.error('Markdown parsing error:', error);
        return `<p>Error rendering markdown: ${error}</p>`;
      }
    } else if (lang === 'html') {
      return text;
    }
    return '';
  }

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
      console.log('[EditorPane] Switching to different note');
      console.log('[EditorPane] From note:', previousNoteId, 'to note:', $selectedNote.id);
      console.log('[EditorPane] New note showPreview value from DB:', $selectedNote.showPreview);

      // Exit draft mode if we were in it
      if ($isDraftMode) {
        console.log('[EditorPane] Exiting draft mode - note selected');
        exitDraftMode();
      }

      // Save and create version for the previous note before switching
      if (previousNoteId && !$isDraftMode) {
        console.log('[EditorPane] Note switch - hasContentChanged:', hasContentChanged);

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
              console.log('[EditorPane] Saved previous note before switching');

              // Create version snapshot
              console.log('[EditorPane] Creating version snapshot (content was modified)');
              await createVersionSnapshot(noteIdToSave);
            } catch (error) {
              console.error('[EditorPane] Error saving/versioning before switch:', error);
            }
          })();
        } else {
          console.log('[EditorPane] Skipping save and version (no content changes)');
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

      console.log('[EditorPane] Local showPreview set to:', showPreview);

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
      console.log('[EditorPane] Closing editor - hasContentChanged:', hasContentChanged);

      // Save immediately (don't wait for debounce)
      if (saveTimeout) {
        clearTimeout(saveTimeout);
        saveTimeout = null;
      }

      // Only save and create version if content was actually modified
      if (hasContentChanged) {
        // Perform the save, then create version
        (async () => {
          try {
            // Save current changes immediately
            await noteService.updateNote(previousNoteId, {
              content,
              tags: tags,
              attachments: attachments,
              syntaxLanguage: language,
              wordWrap,
              showPreview,
            });
            console.log('[EditorPane] Saved pending changes before closing');

            // Create version snapshot
            console.log('[EditorPane] Creating version snapshot (content was modified)');
            await createVersionSnapshot(previousNoteId);

            await triggerBackgroundSync();
          } catch (error) {
            console.error('[EditorPane] Error during close save:', error);
          }
        })();
      } else {
        console.log('[EditorPane] Skipping save and version (no content changes)');
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
        console.log('[EditorPane] Skipping version creation - no changes since last version');
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
      console.log('[EditorPane] Created version snapshot for note:', targetNoteId, 'version:', currentNote.version);
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
    console.log('[EditorPane] handleDelete called');
    if (!$selectedNote) {
      console.log('[EditorPane] No selected note, returning');
      return;
    }
    console.log('[EditorPane] Selected note ID:', $selectedNote.id);

    try {
      console.log('[EditorPane] Calling noteService.deleteNote...');
      const noteId = $selectedNote.id;
      await noteService.deleteNote(noteId);
      console.log('[EditorPane] Delete successful, clearing selection...');
      handleClose(); // Clear selection and return to list on mobile

      // Remove note from store (incremental update)
      notes.update(allNotes => allNotes.filter(n => n.id !== noteId));

      // Remove from search index
      searchService.removeNote(noteId);

      console.log('[EditorPane] Delete operation complete');
    } catch (error) {
      console.error('[EditorPane] Failed to delete note:', error);
    }
  }

  function handleLanguageChange(event: Event) {
    const target = event.target as HTMLSelectElement;
    language = target.value as typeof language;
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
    console.log('[EditorPane] Preview toggled to:', showPreview);
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

  // Check if attachment can be previewed
  function canPreviewAttachment(mimeType: string): boolean {
    if (mimeType.startsWith('image/')) return true;
    if (mimeType.startsWith('text/')) return true;
    if (mimeType.includes('json')) return true;
    if (mimeType.includes('javascript')) return true;
    if (mimeType.includes('xml')) return true;
    if (mimeType.includes('pdf')) return true;
    if (mimeType.startsWith('audio/')) return true;
    if (mimeType.startsWith('video/')) return true;
    return false;
  }

  // Get preview type for attachment
  function getAttachmentPreviewType(mimeType: string): typeof previewType {
    if (mimeType.startsWith('image/')) return 'image';
    if (mimeType.startsWith('audio/')) return 'audio';
    if (mimeType.startsWith('video/')) return 'video';
    if (mimeType.includes('pdf')) return 'pdf';
    if (mimeType.startsWith('text/') ||
        mimeType.includes('json') ||
        mimeType.includes('javascript') ||
        mimeType.includes('xml')) return 'text';
    return 'unsupported';
  }

  // Show attachment preview
  async function handlePreviewAttachment(attachment: Attachment) {
    if (!canPreviewAttachment(attachment.mimeType)) {
      // If can't preview, just download
      await handleDownloadAttachment(attachment);
      return;
    }

    isLoadingPreview = true;
    previewAttachment = attachment;
    previewType = getAttachmentPreviewType(attachment.mimeType);
    previewContent = null;

    try {
      // Get the decrypted data (returns Blob)
      const blob = await attachmentService.getAttachmentData(attachment);

      if (previewType === 'image' || previewType === 'audio' || previewType === 'video' || previewType === 'pdf') {
        // Create blob URL for media files
        previewContent = URL.createObjectURL(blob);
      } else if (previewType === 'text') {
        // Convert blob to text for text files
        const arrayBuffer = await blob.arrayBuffer();
        const text = new TextDecoder().decode(arrayBuffer);
        previewContent = text;
      }
    } catch (error) {
      console.error('Failed to load preview:', error);
      previewType = 'unsupported';
    } finally {
      isLoadingPreview = false;
    }
  }

  // Close attachment preview
  function closeAttachmentPreview() {
    // Revoke blob URLs to free memory
    if (previewContent && (previewType === 'image' || previewType === 'audio' || previewType === 'video' || previewType === 'pdf')) {
      URL.revokeObjectURL(previewContent);
    }

    previewAttachment = null;
    previewContent = null;
    previewType = null;
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

  // Download from preview
  async function handleDownloadFromPreview() {
    if (previewAttachment) {
      await handleDownloadAttachment(previewAttachment);
    }
  }

  async function handleCopy() {
    if (!content) return;

    try {
      await navigator.clipboard.writeText(content);
      // Could show a toast notification here
      console.log('Note content copied to clipboard');
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
        console.log('Note content copied to clipboard (fallback)');
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

    console.log(`Note exported as ${filename}.${extension}`);
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
        console.log(`[Preview] Looking for attachment with filename: ${filename}`);
        console.log(`[Preview] Available attachments:`, attachments.length);

        for (const attachment of attachments) {
          try {
            const encryptedFilename = JSON.parse(attachment.filename);
            const decryptedFilename = await cryptoService.decryptText(encryptedFilename, masterKey.key);
            console.log(`[Preview] Checking attachment ${attachment.id}: "${decryptedFilename}"`);

            if (decryptedFilename === filename) {
              console.log(`[Preview] Found match! ID: ${attachment.id}, data: ${attachment.data}`);
              foundAttachment = attachment;
              break;
            }
          } catch (err) {
            console.error('Failed to decrypt filename for attachment:', attachment.id, err);
          }
        }

        if (foundAttachment) {
          console.log(`[Preview] Loading blob for attachment: ${foundAttachment.id}`);
          // Load and decrypt the blob using attachmentService
          const blob = await attachmentService.getAttachmentData(foundAttachment);
          const blobUrl = URL.createObjectURL(blob);
          console.log(`[Preview] Created blob URL: ${blobUrl} for ${filename}`);
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
      const attachmentData = div.getAttribute('data-attachment-data');
      if (!attachmentId) continue;

      // Skip if already processed
      if (div.classList.contains('clickable')) continue;

      div.classList.add('clickable');
      div.style.cursor = 'pointer';

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
            <span class="font-medium text-red-700 dark:text-red-400">${get(_)('editor.errors.loadingAttachment')}</span>
            <span class="ml-2 text-sm text-gray-600 dark:text-gray-400">${filename}</span>
          </div>
        `;

        div.classList.remove('bg-gray-50', 'dark:bg-gray-800', 'hover:bg-gray-100', 'dark:hover:bg-gray-700', 'cursor-pointer', 'border-gray-300', 'dark:border-gray-600');
        div.classList.add('bg-red-50', 'dark:bg-red-900/20', 'border-red-300', 'dark:border-red-700', 'cursor-not-allowed');
      }
    }
  });
</script>

<svelte:window on:click={handleClickOutside} />

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
      onContentChange={(newContent) => handleInput()}
      onTagsChange={(newTags) => handleInput()}
      onTagClick={handleTagClick}
      {previewHtml}
    />

    <!-- Attachments Section - Only show if attachments exist or dragging files (hidden on mobile) -->
    {#if !forceMobileLayout && (attachments.length > 0 || isDraggingFile)}
      <div class="border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50">
        <!-- Header - Always visible -->
        <button
          on:click={toggleAttachments}
          class="w-full px-3 py-2 flex items-center justify-between text-sm font-medium text-gray-700 dark:text-gray-300 active:bg-gray-100 dark:active:bg-gray-700 transition-colors"
        >
          <span>
            📎 {$_('editor.attachmentsHeader', { values: { count: attachments.length } })}
          </span>
          <span class="text-xs transform transition-transform {isAttachmentsExpanded ? 'rotate-180' : ''}">
            ▼
          </span>
        </button>

        <!-- Expanded content or drop zone -->
        {#if isAttachmentsExpanded || isDraggingFile}
          <div class="px-3 pb-3 max-h-64 overflow-y-auto space-y-3">
            <!-- File Upload - Show when expanded or dragging -->
            <FileUpload
              onUpload={handleFileUpload}
              disabled={isUploading}
            />

            <!-- Attachment List -->
            {#if attachments.length > 0}
              <AttachmentList
                {attachments}
                onDelete={handleDeleteAttachment}
                readonly={false}
              />
            {/if}

            {#if isUploading}
              <div class="text-sm text-blue-600 dark:text-blue-400">
                {$_('editor.uploading')}
              </div>
            {/if}
          </div>
        {:else if attachments.length > 0}
          <!-- Collapsed view - show attachment count -->
          <div class="px-3 pb-2 text-xs text-gray-500 dark:text-gray-400">
            {$_(isAttachmentsExpanded ? 'attachments.clickToHide' : 'attachments.clickToView')}
          </div>
        {/if}
      </div>
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
  <div
    class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4"
    on:click={() => showInfoModal = false}
    on:keydown={(e) => e.key === 'Enter' && (showInfoModal = false)}
    role="button"
    tabindex="-1"
    aria-label="Close modal"
  >
    <div
      class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full p-6"
      on:click|stopPropagation
      on:keydown|stopPropagation
      role="dialog"
      aria-modal="true"
      tabindex="0"
    >
      <div class="flex items-center justify-between mb-4">
        <h2 class="text-xl font-semibold text-gray-900 dark:text-gray-100">{$_('editor.noteInformation')}</h2>
        <button
          on:click={() => showInfoModal = false}
          class="p-1 hover:bg-gray-100 dark:hover:bg-gray-700 rounded"
          aria-label={$_('common.close')}
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <div class="space-y-3 text-sm">
        <div class="grid grid-cols-2 gap-2">
          <div class="text-gray-600 dark:text-gray-400">{$_('editor.characters')}</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{noteStats.characters.toLocaleString()}</div>

          <div class="text-gray-600 dark:text-gray-400">{$_('editor.charactersNoSpaces')}</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{noteStats.charactersNoSpaces.toLocaleString()}</div>

          <div class="text-gray-600 dark:text-gray-400">{$_('editor.words')}</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{noteStats.words.toLocaleString()}</div>

          <div class="text-gray-600 dark:text-gray-400">{$_('editor.lines')}</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{noteStats.lines.toLocaleString()}</div>

          <div class="text-gray-600 dark:text-gray-400">{$_('editor.tags')}</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{noteStats.tags}</div>

          <div class="text-gray-600 dark:text-gray-400">{$_('editor.attachmentsCount')}</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{noteStats.attachments}</div>
        </div>

        <div class="border-t border-gray-200 dark:border-gray-700 pt-3 mt-3">
          <div class="text-gray-600 dark:text-gray-400 mb-2">{$_('editor.syntax')}</div>
          <div class="font-medium text-gray-900 dark:text-gray-100 capitalize">{language.replace('-', ' ')}</div>
        </div>

        {#if $selectedNote}
        <div class="border-t border-gray-200 dark:border-gray-700 pt-3 mt-3">
          <div class="text-gray-600 dark:text-gray-400 mb-2">{$_('note.created')}</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{createdAtFormatted ? $createdAtFormatted : ''}</div>
        </div>

        <div>
          <div class="text-gray-600 dark:text-gray-400 mb-2">{$_('note.modified')}</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{modifiedAtFormatted ? $modifiedAtFormatted : ''}</div>
        </div>
        {/if}

        {#if $selectedNote}
        <div>
          <div class="text-gray-600 dark:text-gray-400 mb-2">{$_('versionHistory.version')}</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">v{$selectedNote.version}</div>
        </div>

        <div>
          <div class="text-gray-600 dark:text-gray-400 mb-2">{$_('editor.noteId')}</div>
          <div class="font-mono text-xs text-gray-900 dark:text-gray-100 break-all">{$selectedNote.id}</div>
        </div>
        {/if}
      </div>

      <div class="mt-6 flex justify-end">
        <button
          on:click={() => showInfoModal = false}
          class="px-4 py-2 min-h-11 bg-blue-600 text-white rounded-md hover:bg-blue-700 transition-colors"
        >
          {$_('common.close')}
        </button>
      </div>
    </div>
  </div>
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
{#if previewAttachment}
  <div
    class="fixed inset-0 bg-black/80 flex items-center justify-center z-50 p-4"
    on:click={closeAttachmentPreview}
    on:keydown={(e) => e.key === 'Enter' && closeAttachmentPreview()}
    role="button"
    tabindex="-1"
    aria-label="Close preview"
  >
    <div
      class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-5xl w-full max-h-[90vh] flex flex-col"
      on:click|stopPropagation
      on:keydown|stopPropagation
      role="dialog"
      aria-modal="true"
      tabindex="0"
    >
      <!-- Header -->
      <div class="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
        <div class="flex-1 min-w-0">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100 truncate">
            {$_('editor.attachmentPreview')}
          </h2>
        </div>
        <div class="flex gap-2 ml-4">
          <button
            on:click={handleDownloadFromPreview}
            class="px-3 py-1.5 bg-blue-600 text-white rounded-md hover:bg-blue-700 transition-colors text-sm"
          >
            {$_('editor.download')}
          </button>
          <button
            on:click={closeAttachmentPreview}
            class="p-1.5 hover:bg-gray-100 dark:hover:bg-gray-700 rounded"
            aria-label="Close"
          >
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
      </div>

      <!-- Content -->
      <div class="flex-1 overflow-auto {previewType === 'image' ? '' : 'p-4'}">
        {#if isLoadingPreview}
          <div class="flex items-center justify-center h-full">
            <div class="text-gray-500 dark:text-gray-400">{$_('editor.status.loadingPreview')}</div>
          </div>
        {:else if previewType === 'image' && previewContent}
          <div class="w-full h-full flex items-center justify-center p-4">
            <img
              src={previewContent}
              alt="Attachment preview"
              class="max-w-full max-h-full object-contain"
              style="max-height: calc(90vh - 120px);"
            />
          </div>
        {:else if previewType === 'text' && previewContent}
          <pre class="text-sm bg-gray-50 dark:bg-gray-900 p-4 rounded overflow-auto max-h-full"><code class="text-gray-900 dark:text-gray-100">{previewContent}</code></pre>
        {:else if previewType === 'pdf' && previewContent}
          <PdfViewer
            pdfUrl={previewContent}
            filename="attachment.pdf"
          />
        {:else if previewType === 'audio' && previewContent}
          <div class="flex items-center justify-center h-full">
            <audio controls class="w-full max-w-xl">
              <source src={previewContent} type={previewAttachment.mimeType} />
              Your browser does not support audio playback.
            </audio>
          </div>
        {:else if previewType === 'video' && previewContent}
          <div class="flex items-center justify-center h-full">
            <video controls class="max-w-full max-h-full" aria-label="Video preview - captions not available for user-uploaded content">
              <source src={previewContent} type={previewAttachment.mimeType} />
              <track kind="captions" />
              {$_('editor.errors.videoNotSupported')}
            </video>
          </div>
        {:else}
          <div class="flex items-center justify-center h-full">
            <div class="text-center text-gray-500 dark:text-gray-400">
              <p class="mb-4">{$_('editor.errors.previewNotAvailable')}</p>
              <button
                on:click={handleDownloadFromPreview}
                class="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 transition-colors"
              >
                {$_('editor.errors.downloadToView')}
              </button>
            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<!-- Attachments Modal (Mobile) -->
{#if showAttachmentsModal && forceMobileLayout}
  <div
    class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4"
    on:click={() => showAttachmentsModal = false}
    on:keydown={(e) => e.key === 'Enter' && (showAttachmentsModal = false)}
    role="button"
    tabindex="-1"
    aria-label="Close attachments"
  >
    <div
      class="bg-white dark:bg-gray-800 rounded-lg shadow-xl w-full max-w-2xl max-h-[80vh] flex flex-col"
      on:click|stopPropagation
      on:keydown|stopPropagation
      role="dialog"
      aria-modal="true"
      tabindex="0"
    >
      <!-- Header -->
      <div class="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100">
          {$_('editor.attachments')} ({attachments.length})
        </h2>
        <button
          on:click={() => showAttachmentsModal = false}
          class="p-2 hover:bg-gray-100 dark:hover:bg-gray-700 rounded transition-colors"
          aria-label={$_('common.close')}
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <!-- Content -->
      <div class="flex-1 overflow-y-auto p-4">
        {#if attachments.length > 0}
          <AttachmentList
            {attachments}
            readonly={false}
            onDelete={handleDeleteAttachment}
            onPreview={handlePreviewAttachment}
          />
        {:else}
          <div class="text-center text-gray-500 dark:text-gray-400 py-8">
            {$_('attachments.noAttachments')}
          </div>
        {/if}
      </div>

      <!-- Footer with Add Button -->
      <div class="border-t border-gray-200 dark:border-gray-700 p-4">
        <FileUpload
          onUpload={handleFileUpload}
          disabled={isUploading}
        />
      </div>
    </div>
  </div>
{/if}
