<script lang="ts">
  import type { DecryptedNote } from '../../types';
  import { tagService } from '../../services';

  export let showDocumentation: boolean;
  export let onShowDocumentation: () => void;
  export let stats: { total: number; active: number; deleted: number; pinned: number; } | null = null;
  export let notes: DecryptedNote[] = [];

  // Access the version from the global __APP_VERSION__ variable
  declare const __APP_VERSION__: string;

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

  $: maxCount = popularTags.length > 0 ? popularTags[0].count : 0;
</script>

<div class="space-y-6">
  <!-- Version -->
  <div>
    <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Version</h4>
    <p class="text-lg text-gray-900 dark:text-white font-mono">v{__APP_VERSION__}</p>
  </div>

  <!-- Statistics -->
  {#if stats}
  <div>
    <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">Statistics</h4>
    <div class="grid grid-cols-2 gap-3">
      <div class="bg-blue-50 dark:bg-blue-900/20 rounded-lg p-3 border border-blue-200 dark:border-blue-800">
        <div class="text-2xl font-bold text-blue-600 dark:text-blue-400">{stats.active}</div>
        <div class="text-sm text-blue-700 dark:text-blue-300">Active Notes</div>
      </div>
      <div class="bg-yellow-50 dark:bg-yellow-900/20 rounded-lg p-3 border border-yellow-200 dark:border-yellow-800">
        <div class="text-2xl font-bold text-yellow-600 dark:text-yellow-400">{stats.pinned}</div>
        <div class="text-sm text-yellow-700 dark:text-yellow-300">Pinned Notes</div>
      </div>
      <div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-3 border border-gray-200 dark:border-gray-700">
        <div class="text-2xl font-bold text-gray-600 dark:text-gray-400">{stats.deleted}</div>
        <div class="text-sm text-gray-700 dark:text-gray-300">In Recycle Bin</div>
      </div>
      <div class="bg-purple-50 dark:bg-purple-900/20 rounded-lg p-3 border border-purple-200 dark:border-purple-800">
        <div class="text-2xl font-bold text-purple-600 dark:text-purple-400">{stats.total}</div>
        <div class="text-sm text-purple-700 dark:text-purple-300">Total Notes</div>
      </div>
    </div>
  </div>
  {/if}

  <!-- Tag Cloud -->
  {#if popularTags.length > 0}
  <div>
    <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">Popular Tags</h4>
    <div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-4 border border-gray-200 dark:border-gray-700">
      <div class="flex flex-wrap gap-2">
        {#each popularTags as { tag, count }}
          <span
            class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-blue-100 dark:bg-blue-900/30 text-blue-800 dark:text-blue-200 font-medium transition-all hover:bg-blue-200 dark:hover:bg-blue-900/50 {getTagSize(count, maxCount)} {getTagOpacity(count, maxCount)}"
            title="{count} note{count === 1 ? '' : 's'}"
          >
            <span class="text-blue-600 dark:text-blue-400">#</span>{tag}
            <span class="text-xs opacity-75">({count})</span>
          </span>
        {/each}
      </div>
    </div>
  </div>
  {/if}

  <!-- Documentation -->
  <div>
    <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">Documentation</h4>
    <button
      on:click={onShowDocumentation}
      class="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-md transition-colors flex items-center justify-center gap-2"
    >
      📚 View Documentation
    </button>
    <p class="mt-2 text-sm text-gray-500 dark:text-gray-400">
      Learn how to use Jottery effectively
    </p>
  </div>

  <!-- Terminal Client Info -->
  <div>
    <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Terminal Client</h4>
    <p class="text-sm text-gray-600 dark:text-gray-400 mb-2">
      Access your notes from the command line with the Jottery terminal client. The TUI provides a fast, keyboard-driven interface for managing your notes.
    </p>
    <p class="text-sm text-gray-600 dark:text-gray-400">
      Download the terminal client from the <strong>Advanced</strong> tab.
    </p>
  </div>

  <!-- About -->
  <div>
    <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">About Jottery</h4>
    <p class="text-sm text-gray-600 dark:text-gray-400 mb-3">
      Jottery is a privacy-focused, self-hosted scratch pad application for capturing, organizing, and searching notes with rich content, syntax highlighting, and encryption.
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
      View on GitHub
    </a>
    <p class="text-sm text-gray-600 dark:text-gray-400">
      Licensed under the MIT License.
    </p>
  </div>
</div>
