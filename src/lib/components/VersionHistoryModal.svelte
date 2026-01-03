<script lang="ts">
  import { onMount } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { versionRepository, noteRepository, cryptoService, keyManager, decryptJSON } from '../services';
  import type { DecryptedNoteVersion, NoteVersion } from '../types';
  import { formatDate } from '../utils/dateFormat';
  import ConfirmModal from './ConfirmModal.svelte';
  import { toast } from '../utils/toast.svelte';

  export let show: boolean = false;
  export let noteId: string | undefined = undefined;
  export let currentVersion: number | undefined = undefined;
  export let onClose: () => void;
  export let onRestore: ((version: number) => Promise<void>) | undefined = undefined;

  let versions: DecryptedNoteVersion[] = [];
  let loading = false;
  let selectedVersion: DecryptedNoteVersion | null = null;
  let showRestoreConfirm = false;
  let versionToRestore: number | null = null;

  async function decryptVersion(version: NoteVersion): Promise<DecryptedNoteVersion> {
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) throw new Error('Application is locked');

    const encryptedContent = JSON.parse(version.content);
    const content = await cryptoService.decryptText(encryptedContent, masterKey.key);

    let tags: string[] = [];
    if (version.tags.length > 0) {
      if (version.tags.length === 1) {
        const encryptedTags = JSON.parse(version.tags[0]);
        tags = await decryptJSON(encryptedTags, masterKey.key);
      } else {
        // Old format - individually encrypted tags
        for (const tagJson of version.tags) {
          const encryptedTag = JSON.parse(tagJson);
          const tagText = await cryptoService.decryptText(encryptedTag, masterKey.key);
          tags.push(JSON.parse(tagText));
        }
      }
    }

    return {
      ...version,
      content,
      tags,
      characterCount: content.length
    };
  }

  async function loadVersions() {
    if (!noteId) return;

    loading = true;
    try {
      const encryptedVersions = await versionRepository.getVersionsForNote(noteId);
      versions = await Promise.all(encryptedVersions.map(v => decryptVersion(v)));
      // Auto-select the first (newest) version
      if (versions.length > 0) {
        selectedVersion = versions[0];
      }
    } catch (error) {
      console.error('Failed to load versions:', error);
    } finally {
      loading = false;
    }
  }

  function handleSelectVersion(version: DecryptedNoteVersion) {
    selectedVersion = version;
  }

  function handleRestoreClick() {
    if (!selectedVersion) return;
    versionToRestore = selectedVersion.version;
    showRestoreConfirm = true;
  }

  async function confirmRestore() {
    showRestoreConfirm = false;
    if (versionToRestore === null || !noteId) return;

    try {
      if (onRestore) {
        await onRestore(versionToRestore);
      } else {
        // Restore the version directly
        const version = await versionRepository.getVersion(noteId, versionToRestore);
        if (!version) throw new Error(`Version ${versionToRestore} not found`);

        const currentNote = await noteRepository.getById(noteId);
        if (!currentNote) throw new Error(`Note ${noteId} not found`);

        // Snapshot current state before restoring
        await versionRepository.createVersion(currentNote, {
          syncedAt: new Date().toISOString(),
          reason: 'manual-sync',
        });

        // Restore version data
        currentNote.content = version.content;
        currentNote.tags = version.tags;
        currentNote.attachments = version.attachments;
        currentNote.syntaxLanguage = version.syntaxLanguage;
        currentNote.wordWrap = version.wordWrap;

        await noteRepository.update(currentNote);
      }
      versionToRestore = null;
      onClose();
    } catch (error) {
      console.error('Failed to restore version:', error);
      toast.error('Failed to restore version: ' + (error instanceof Error ? error.message : String(error)));
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      onClose();
    }
  }

  function getPreviewContent(content: string): string {
    // Return full content for scrollable preview
    return content;
  }

  function formatReason(reason: string): string {
    return reason === 'sync' ? $_('versionHistory.autoSync') : $_('versionHistory.manualSync');
  }

  // Load versions when modal opens or noteId changes
  $: if (show && noteId) {
    loadVersions();
  }

  // Reactive date formatting stores for selected version
  $: selectedVersionCreatedAt = selectedVersion ? formatDate(selectedVersion.createdAt, { month: 'short', day: 'numeric', year: 'numeric', hour: '2-digit', minute: '2-digit' }) : null;
  $: selectedVersionSyncedAt = selectedVersion ? formatDate(selectedVersion.syncedAt, { month: 'short', day: 'numeric', year: 'numeric', hour: '2-digit', minute: '2-digit' }) : null;
</script>

{#if show}
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-0 tablet:p-4"
    on:click={handleBackdropClick}
    on:keydown={(e) => e.key === 'Escape' && onClose()}
    role="dialog"
    aria-modal="true"
    tabindex="0"
  >
    <div class="bg-white dark:bg-gray-800 w-full h-full tablet:h-auto tablet:max-w-6xl tablet:rounded-lg shadow-xl tablet:max-h-[90vh] flex flex-col">
      <!-- Header -->
      <div class="border-b border-gray-200 dark:border-gray-700 p-4 flex items-center justify-between flex-shrink-0">
        <h2 class="text-xl font-bold text-gray-900 dark:text-white">{$_('versionHistory.title')}</h2>
        <button
          on:click={onClose}
          class="min-h-11 min-w-11 p-3 -m-2 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors text-gray-500 dark:text-gray-400"
          aria-label={$_('versionHistory.closeLabel')}
        >
          ✕
        </button>
      </div>

      <!-- Content: Two-pane layout -->
      <div class="flex-1 flex overflow-hidden">
        {#if loading}
          <div class="flex-1 flex items-center justify-center py-8">
            <div class="text-center">
              <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600 mx-auto"></div>
              <p class="mt-2 text-gray-600 dark:text-gray-400">{$_('versionHistory.loading')}</p>
            </div>
          </div>
        {:else if versions.length === 0}
          <div class="flex-1 flex items-center justify-center py-8">
            <div class="text-center">
              <p class="text-lg text-gray-600 dark:text-gray-400">{$_('versionHistory.noVersions')}</p>
              <p class="text-sm text-gray-500 dark:text-gray-500 mt-1">{$_('versionHistory.versionsCreatedDuringSync')}</p>
            </div>
          </div>
        {:else}
          <!-- Left pane: Version list -->
          <div class="w-64 border-r border-gray-200 dark:border-gray-700 overflow-y-auto flex-shrink-0">
            <div class="p-2">
              {#each versions as version (version.versionKey)}
                <button
                  on:click={() => handleSelectVersion(version)}
                  class="w-full text-left p-3 rounded-md mb-1 transition-colors {selectedVersion?.versionKey === version.versionKey ? 'bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800' : 'hover:bg-gray-100 dark:hover:bg-gray-700'}"
                >
                  <div class="flex items-center justify-between">
                    <span class="font-medium text-gray-900 dark:text-gray-100">
                      v{version.version}
                      {#if currentVersion === version.version}
                        <span class="text-xs text-blue-600 dark:text-blue-400 ml-1">{$_('versionHistory.current')}</span>
                      {/if}
                    </span>
                  </div>
                  <div class="text-xs text-gray-600 dark:text-gray-400 mt-1">
                    {@const createdDateStore = formatDate(version.createdAt, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })}
                    {$createdDateStore}
                  </div>
                  <div class="text-xs text-gray-500 dark:text-gray-500 mt-0.5">
                    {formatReason(version.reason)}
                  </div>
                </button>
              {/each}
            </div>
          </div>

          <!-- Right pane: Preview -->
          <div class="flex-1 flex flex-col overflow-hidden">
            {#if selectedVersion}
              <div class="flex-1 p-4 flex flex-col overflow-hidden">
                <div class="flex-1 flex flex-col space-y-4 min-h-0">
                  <!-- Metadata -->
                  <div class="bg-gray-50 dark:bg-gray-900 rounded-md p-3 text-sm flex-shrink-0">
                    <div class="grid grid-cols-2 gap-2">
                      <div>
                        <span class="text-gray-600 dark:text-gray-400">{$_('versionHistory.version')}</span>
                        <span class="ml-2 font-medium text-gray-900 dark:text-gray-100">v{selectedVersion.version}</span>
                      </div>
                      <div>
                        <span class="text-gray-600 dark:text-gray-400">{$_('versionHistory.characters')}</span>
                        <span class="ml-2 font-medium text-gray-900 dark:text-gray-100">{selectedVersion.characterCount?.toLocaleString() || 0}</span>
                      </div>
                      <div>
                        <span class="text-gray-600 dark:text-gray-400">{$_('versionHistory.created')}</span>
                        <span class="ml-2 font-medium text-gray-900 dark:text-gray-100">
                          {selectedVersionCreatedAt ? $selectedVersionCreatedAt : ''}
                        </span>
                      </div>
                      <div>
                        <span class="text-gray-600 dark:text-gray-400">{$_('versionHistory.synced')}</span>
                        <span class="ml-2 font-medium text-gray-900 dark:text-gray-100">
                          {selectedVersionSyncedAt ? $selectedVersionSyncedAt : ''}
                        </span>
                      </div>
                    </div>
                    {#if selectedVersion.tags.length > 0}
                      <div class="mt-2 pt-2 border-t border-gray-200 dark:border-gray-700">
                        <span class="text-gray-600 dark:text-gray-400">{$_('versionHistory.tags')}</span>
                        <div class="flex gap-1 mt-1 flex-wrap">
                          {#each selectedVersion.tags as tag}
                            <span class="px-2 py-0.5 bg-blue-100 dark:bg-blue-900/30 text-blue-800 dark:text-blue-300 rounded text-xs">
                              #{tag}
                            </span>
                          {/each}
                        </div>
                      </div>
                    {/if}
                  </div>

                  <!-- Content Preview -->
                  <div class="flex-1 flex flex-col min-h-0">
                    <h3 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">{$_('versionHistory.contentPreview')}</h3>
                    <div class="bg-gray-50 dark:bg-gray-900 rounded-md p-3 overflow-y-auto flex-1">
                      <pre class="whitespace-pre-wrap text-sm text-gray-900 dark:text-gray-100 font-mono">{getPreviewContent(selectedVersion.content)}</pre>
                    </div>
                  </div>
                </div>
              </div>
            {:else}
              <div class="flex-1 flex items-center justify-center">
                <p class="text-gray-500 dark:text-gray-400">{$_('versionHistory.selectToPreview')}</p>
              </div>
            {/if}
          </div>
        {/if}
      </div>

      <!-- Footer -->
      {#if versions.length > 0}
        <div class="border-t border-gray-200 dark:border-gray-700 p-4 flex justify-end gap-3 flex-shrink-0">
          <button
            on:click={onClose}
            class="px-4 py-2.5 min-h-11 text-gray-700 dark:text-gray-300 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
          >
            {$_('common.close')}
          </button>
          {#if selectedVersion && selectedVersion.version !== currentVersion}
            <button
              on:click={handleRestoreClick}
              class="px-4 py-2.5 min-h-11 bg-blue-600 active:bg-blue-700 text-white font-medium rounded-md transition-colors"
            >
              {$_('versionHistory.restoreButton')}
            </button>
          {/if}
        </div>
      {/if}
    </div>
  </div>

  <!-- Restore Confirmation Modal -->
  <ConfirmModal
    show={showRestoreConfirm}
    title={$_('versionHistory.restoreConfirm.title')}
    message={$_('versionHistory.restoreConfirm.message')}
    confirmText={$_('versionHistory.restoreConfirm.confirmButton')}
    cancelText={$_('common.cancel')}
    confirmClass="bg-blue-600 hover:bg-blue-700"
    onConfirm={confirmRestore}
    onCancel={() => {
      showRestoreConfirm = false;
      versionToRestore = null;
    }}
  />
{/if}
