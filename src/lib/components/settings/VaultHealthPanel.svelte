<script lang="ts">
  import { _ } from 'svelte-i18n';
  import {
    undecryptableNotes,
    scanMissingAttachments,
    repairAttachment,
    deleteUndecryptableNote,
    type MissingAttachment,
  } from '../../services/vaultHealthService';
  import ConfirmModal from '../ConfirmModal.svelte';
  import { toast } from '../../utils/toast.svelte';

  let missing: MissingAttachment[] = [];
  let scanning = false;
  let scanned = false;
  let repairing: Record<string, boolean> = {};
  let repairResult: Record<string, 'ok' | 'fail'> = {};
  let confirmDeleteId: string | null = null;
  let deleting = false;

  async function rescan() {
    scanning = true;
    try {
      missing = await scanMissingAttachments();
      scanned = true;
    } catch (error) {
      console.error('[VaultHealth] Attachment scan failed:', error);
    } finally {
      scanning = false;
    }
  }

  async function repair(id: string) {
    repairing = { ...repairing, [id]: true };
    const ok = await repairAttachment(id);
    repairing = { ...repairing, [id]: false };
    repairResult = { ...repairResult, [id]: ok ? 'ok' : 'fail' };
    if (ok) {
      missing = missing.filter(m => m.attachmentId !== id);
    }
  }

  async function confirmDelete() {
    if (!confirmDeleteId) return;
    deleting = true;
    try {
      await deleteUndecryptableNote(confirmDeleteId);
    } catch (error) {
      console.error('[VaultHealth] Delete failed:', error);
      toast.error($_('vaultHealth.deleteFailed'));
    } finally {
      deleting = false;
      confirmDeleteId = null;
    }
  }

  function formatWhen(iso: string): string {
    try {
      return new Date(iso).toLocaleString();
    } catch {
      return iso;
    }
  }

  function formatSize(bytes: number): string {
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${bytes} B`;
  }

  $: healthy = scanned && !scanning && $undecryptableNotes.length === 0 && missing.length === 0;

  rescan();
</script>

<div class="bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg p-4">
  <div class="flex items-center justify-between mb-2">
    <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300">
      🩺 {$_('vaultHealth.title')}
    </h4>
    <button
      on:click={rescan}
      disabled={scanning}
      class="px-3 py-1 text-xs font-medium rounded-md border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors disabled:opacity-50"
    >
      {scanning ? $_('vaultHealth.scanning') : $_('vaultHealth.rescan')}
    </button>
  </div>
  <p class="text-sm text-gray-500 dark:text-gray-400 mb-3">
    {$_('vaultHealth.description')}
  </p>

  {#if healthy}
    <p class="text-sm text-green-700 dark:text-green-400" data-testid="vault-health-healthy">
      ✓ {$_('vaultHealth.healthy')}
    </p>
  {/if}

  {#if $undecryptableNotes.length > 0}
    <div class="mb-4" data-testid="vault-health-undecryptable">
      <h5 class="text-sm font-medium text-red-800 dark:text-red-200 mb-1">
        {$_('vaultHealth.undecryptableTitle')} ({$undecryptableNotes.length})
      </h5>
      <p class="text-xs text-gray-500 dark:text-gray-400 mb-2">
        {$_('vaultHealth.undecryptableHint')}
      </p>
      <ul class="space-y-2">
        {#each $undecryptableNotes as entry (entry.id)}
          <li class="border border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-900/20 rounded-md p-3 text-sm">
            <div class="flex items-start justify-between gap-3">
              <div class="min-w-0">
                <p class="font-mono text-xs text-gray-500 dark:text-gray-400 truncate">{entry.id}</p>
                <p class="text-xs text-gray-600 dark:text-gray-300 mt-1">
                  {$_('vaultHealth.createdAt')}: {formatWhen(entry.createdAt)}
                  · {$_('vaultHealth.modifiedAt')}: {formatWhen(entry.modifiedAt)}
                  · {$_('vaultHealth.size')}: {formatSize(entry.ciphertextLength)}
                </p>
                <p class="text-xs text-red-700 dark:text-red-300 mt-1 break-words">
                  {$_('vaultHealth.errorLabel')}: {entry.error}
                </p>
              </div>
              <button
                on:click={() => confirmDeleteId = entry.id}
                class="flex-shrink-0 px-3 py-1.5 text-xs font-medium rounded-md bg-red-600 hover:bg-red-700 text-white transition-colors"
              >
                {$_('vaultHealth.delete')}
              </button>
            </div>
          </li>
        {/each}
      </ul>
    </div>
  {/if}

  {#if missing.length > 0}
    <div data-testid="vault-health-missing">
      <h5 class="text-sm font-medium text-amber-800 dark:text-amber-200 mb-1">
        {$_('vaultHealth.missingTitle')} ({missing.length})
      </h5>
      <p class="text-xs text-gray-500 dark:text-gray-400 mb-2">
        {$_('vaultHealth.missingHint')}
      </p>
      <ul class="space-y-2">
        {#each missing as item (item.attachmentId)}
          <li class="border border-amber-200 dark:border-amber-800 bg-amber-50 dark:bg-amber-900/20 rounded-md p-3 text-sm">
            <div class="flex items-center justify-between gap-3">
              <div class="min-w-0">
                <p class="truncate">
                  {$_('vaultHealth.noteLabel')}: {item.noteTitle ?? $_('vaultHealth.unknownNote')}
                </p>
                <p class="font-mono text-xs text-gray-500 dark:text-gray-400 mt-1">{item.attachmentId.slice(0, 8)}…</p>
                {#if repairResult[item.attachmentId] === 'fail'}
                  <p class="text-xs text-red-700 dark:text-red-300 mt-1">{$_('vaultHealth.repairFailed')}</p>
                {/if}
              </div>
              <button
                on:click={() => repair(item.attachmentId)}
                disabled={repairing[item.attachmentId]}
                class="flex-shrink-0 px-3 py-1.5 text-xs font-medium rounded-md bg-amber-600 hover:bg-amber-700 text-white transition-colors disabled:opacity-50"
              >
                {$_('vaultHealth.repair')}
              </button>
            </div>
          </li>
        {/each}
      </ul>
    </div>
  {/if}
</div>

<ConfirmModal
  show={confirmDeleteId !== null}
  title={$_('vaultHealth.deleteConfirmTitle')}
  message={$_('vaultHealth.deleteConfirmMessage')}
  confirmText={deleting ? '…' : $_('vaultHealth.delete')}
  cancelText={$_('common.cancel')}
  onConfirm={confirmDelete}
  onCancel={() => confirmDeleteId = null}
/>
