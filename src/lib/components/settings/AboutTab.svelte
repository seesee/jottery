<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { DecryptedNote } from '../../types';
  import { tagService } from '../../services';
  import { searchQuery } from '../../stores/appStore';

  export let _showDocumentation: boolean;
  export let onShowDocumentation: () => void;
  export let onClose: () => void;
  export let stats: { total: number; active: number; deleted: number; pinned: number; } | null = null;
  export let notes: DecryptedNote[] = [];

  // Calculate popular tags
  $: popularTags = tagService.getPopularTags(notes, 20);

  // Get font size for tag based on count (relative to max count)
  function getTagSize(count: number, maxCount: number): string {
    if (maxCount === 0) return 'text-sm';
    const ratio = count / maxCount;
    if (ratio >= 0.8) return 'text-xl';
    if (ratio >= 0.6) return 'text-lg';
    if (ratio >= 0.4) return 'text-base';
    return 'text-sm';
  }

  // Get opacity for tag based on count
  function getTagOpacity(count: number, maxCount: number): string {
    if (maxCount === 0) return 'opacity-70';
    const ratio = count / maxCount;
    if (ratio >= 0.8) return 'opacity-100';
    if (ratio >= 0.6) return 'opacity-90';
    if (ratio >= 0.4) return 'opacity-80';
    return 'opacity-70';
  }

  // Handle tag click - search for tag and close modal
  function handleTagClick(tag: string) {
    searchQuery.set(`#${tag}`);
    onClose();
  }

  $: maxCount = popularTags.length > 0 ? popularTags[0].count : 0;
</script>

<div class="space-y-6">
  <!-- Version -->
  <div>
    <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">{$_('settings.aboutTab.version')}</h4>
    <p class="text-lg text-gray-900 dark:text-white font-mono">v{__APP_VERSION__}</p>
  </div>

  <!-- Documentation -->
  <div>
    <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">{$_('settings.aboutTab.documentation')}</h4>
    <button
      on:click={onShowDocumentation}
      class="w-full px-4 py-2 min-h-11 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-md transition-colors flex items-center justify-center gap-2"
    >
      📚 {$_('settings.aboutTab.viewDocumentation')}
    </button>
    <p class="mt-2 text-sm text-gray-500 dark:text-gray-400">
      {$_('settings.aboutTab.documentationDescription')}
    </p>
  </div>

  <!-- Statistics -->
  {#if stats}
  <div>
    <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">{$_('settings.aboutTab.statistics')}</h4>
    <div class="grid grid-cols-2 gap-3">
      <div class="bg-blue-50 dark:bg-blue-900/20 rounded-lg p-3 border border-blue-200 dark:border-blue-800">
        <div class="text-2xl font-bold text-blue-600 dark:text-blue-400">{stats.active}</div>
        <div class="text-sm text-blue-700 dark:text-blue-300">{$_('settings.aboutTab.activeNotes')}</div>
      </div>
      <div class="bg-yellow-50 dark:bg-yellow-900/20 rounded-lg p-3 border border-yellow-200 dark:border-yellow-800">
        <div class="text-2xl font-bold text-yellow-600 dark:text-yellow-400">{stats.pinned}</div>
        <div class="text-sm text-yellow-700 dark:text-yellow-300">{$_('settings.aboutTab.pinnedNotes')}</div>
      </div>
      <div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-3 border border-gray-200 dark:border-gray-700">
        <div class="text-2xl font-bold text-gray-600 dark:text-gray-400">{stats.deleted}</div>
        <div class="text-sm text-gray-700 dark:text-gray-300">{$_('settings.aboutTab.inRecycleBin')}</div>
      </div>
      <div class="bg-purple-50 dark:bg-purple-900/20 rounded-lg p-3 border border-purple-200 dark:border-purple-800">
        <div class="text-2xl font-bold text-purple-600 dark:text-purple-400">{stats.total}</div>
        <div class="text-sm text-purple-700 dark:text-purple-300">{$_('settings.aboutTab.totalNotes')}</div>
      </div>
    </div>
  </div>
  {/if}

  <!-- Tag Cloud -->
  {#if popularTags.length > 0}
  <div>
    <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">{$_('settings.aboutTab.popularTags')}</h4>
    <div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-4 border border-gray-200 dark:border-gray-700">
      <div class="flex flex-wrap gap-2">
        {#each popularTags as { tag, count }}
          <button
            on:click={() => handleTagClick(tag)}
            class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-blue-100 dark:bg-blue-900/30 text-blue-800 dark:text-blue-200 font-medium transition-all hover:bg-blue-200 dark:hover:bg-blue-900/50 active:bg-blue-300 dark:active:bg-blue-700 cursor-pointer {getTagSize(count, maxCount)} {getTagOpacity(count, maxCount)}"
            title="Click to search for #{tag} ({count} note{count === 1 ? '' : 's'})"
          >
            <span class="text-blue-600 dark:text-blue-400">#</span>{tag}
            <span class="text-xs opacity-75">({count})</span>
          </button>
        {/each}
      </div>
    </div>
  </div>
  {/if}

  <!-- Terminal Client Info -->
  <div>
    <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">{$_('settings.aboutTab.terminalClient')}</h4>
    <p class="text-sm text-gray-600 dark:text-gray-400 mb-2">
      {$_('settings.aboutTab.terminalClientDescription')}
    </p>
    <p class="text-sm text-gray-600 dark:text-gray-400">
      {@html $_('settings.aboutTab.terminalClientDownload')}
    </p>
  </div>

  <!-- About -->
  <div>
    <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">{$_('settings.aboutTab.aboutJottery')}</h4>
    <p class="text-sm text-gray-600 dark:text-gray-400 mb-3">
      {$_('settings.aboutTab.aboutDescription')}
    </p>
    <a
      href="https://github.com/seesee/jottery"
      target="_blank"
      rel="noopener noreferrer"
      class="inline-flex items-center text-blue-600 dark:text-blue-400 hover:text-blue-800 dark:hover:text-blue-300 font-medium text-sm mb-3"
    >
      <svg class="w-4 h-4 mr-1.5" fill="currentColor" viewBox="0 0 24 24">
        <path fill-rule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clip-rule="evenodd"/>
      </svg>
      {$_('settings.aboutTab.viewOnGithub')}
    </a>
    <p class="text-sm text-gray-600 dark:text-gray-400">
      {$_('settings.aboutTab.license')}
    </p>
  </div>
</div>
