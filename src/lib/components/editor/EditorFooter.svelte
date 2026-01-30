<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { Readable } from 'svelte/store';

  export let version: number | undefined = undefined;
  export let modifiedAtFormatted: Readable<string> | null;
  export let backgroundColor: string | undefined = undefined;
  export let isSynced: boolean = false;
  export let syncEnabled: boolean = true;
  export let neverSynced: boolean = false;
  export let onShowVersionHistory: (() => void) | undefined = undefined;

  // Determine sync indicator state:
  // - Sync enabled + synced: green filled
  // - Sync enabled + not synced: red hollow
  // - Sync disabled + previously synced: green filled
  // - Sync disabled + never synced: grey hollow
  $: syncState = isSynced ? 'synced'
    : (!syncEnabled && neverSynced) ? 'never-synced-disabled'
    : 'unsynced';

  $: indicatorClass = {
    'synced': 'text-green-500',
    'unsynced': 'text-red-500',
    'never-synced-disabled': 'text-gray-400'
  }[syncState];

  $: indicatorTitle = {
    'synced': $_('note.synced'),
    'unsynced': $_('note.unsynced'),
    'never-synced-disabled': $_('note.neverSynced')
  }[syncState];

  $: indicatorSymbol = syncState === 'synced' ? '●' : '○';
</script>

<!-- Metadata Footer -->
<div
  class="border-t border-gray-200 dark:border-gray-700 p-2 text-xs text-gray-500 dark:text-gray-400"
  style:background-color={backgroundColor}
>
  <div class="flex justify-between">
    <span class="flex items-center gap-1.5">
      {#if onShowVersionHistory}
        <button
          type="button"
          on:click={onShowVersionHistory}
          class="hover:text-blue-500 dark:hover:text-blue-400 hover:underline cursor-pointer"
          title={$_('versionHistory.title')}
        >
          {$_('note.version')}: v{version ?? 1}
        </button>
      {:else}
        <span>{$_('note.version')}: v{version ?? 1}</span>
      {/if}
      <span
        class={indicatorClass}
        title={indicatorTitle}
      >
        {indicatorSymbol}
      </span>
    </span>
    <span>{$_('note.modified')}: {modifiedAtFormatted ? $modifiedAtFormatted : ''}</span>
  </div>
</div>
