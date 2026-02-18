<script lang="ts">
  import { _ } from 'svelte-i18n';

  export let show = false;
  export let onClose: () => void;
  export let onSelectQuery: (query: string) => void;
  export let recentSearches: string[] = [];
  export let onRemoveRecent: (query: string) => void = () => {};
  export let onClearAllRecent: () => void = () => {};

  const syntaxExamples = [
    { query: '#tag', label: 'searchHelp.tags' },
    { query: '"exact phrase"', label: 'searchHelp.exactPhrase' },
    { query: 'has:attachment', label: 'searchHelp.hasAttachment' },
    { query: 'created:>2024-01-01', label: 'searchHelp.createdAfter' },
    { query: 'words:>100', label: 'searchHelp.wordsMore' },
    { query: 'word1 | word2', label: 'searchHelp.eitherWord' },
    { query: '-excluded', label: 'searchHelp.notContaining' },
    { query: 'dog*', label: 'searchHelp.startsWith' },
  ];

  function handleSelectQuery(query: string) {
    onSelectQuery(query);
    onClose();
  }

  function handleRemoveRecent(e: MouseEvent, query: string) {
    e.stopPropagation();
    onRemoveRecent(query);
  }
</script>

{#if show}
  <div
    data-testid="search-suggestions"
    class="absolute z-50 w-full mt-1 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-600 rounded-lg shadow-xl max-h-72 overflow-y-auto"
    role="listbox"
    aria-label={$_('searchSuggestions.recentSearches')}
  >
    <!-- Recent Searches Section -->
    {#if recentSearches.length > 0}
      <div class="px-3 pt-2.5 pb-1">
        <div class="flex items-center justify-between mb-1.5">
          <h4 class="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wide">
            {$_('searchSuggestions.recentSearches')}
          </h4>
          <button
            on:click|stopPropagation={onClearAllRecent}
            class="text-xs text-gray-400 dark:text-gray-500 hover:text-red-500 dark:hover:text-red-400 transition-colors"
          >
            {$_('searchSuggestions.clearAll')}
          </button>
        </div>
        {#each recentSearches as query}
          <div
            role="option"
            aria-selected="false"
            on:click={() => handleSelectQuery(query)}
            on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && handleSelectQuery(query)}
            tabindex="-1"
            class="flex items-center justify-between group px-2 py-1.5 rounded-md cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
          >
            <div class="flex items-center gap-2 min-w-0">
              <svg class="w-3.5 h-3.5 text-gray-400 dark:text-gray-500 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              <span class="text-sm text-gray-700 dark:text-gray-300 truncate">{query}</span>
            </div>
            <button
              on:click={(e) => handleRemoveRecent(e, query)}
              class="opacity-0 group-hover:opacity-100 text-gray-400 hover:text-red-500 dark:hover:text-red-400 transition-all p-0.5 shrink-0"
              title={$_('searchSuggestions.clearRecent')}
              aria-label="{$_('searchSuggestions.clearRecent')}: {query}"
            >
              <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
        {/each}
      </div>
      <div class="border-t border-gray-100 dark:border-gray-700 mx-3"></div>
    {/if}

    <!-- Syntax Quick-Picks Section -->
    <div class="px-3 pt-2 pb-2.5">
      <h4 class="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-1.5">
        {$_('searchSuggestions.tryThese')}
      </h4>
      <div class="flex flex-wrap gap-1.5">
        {#each syntaxExamples as example}
          <button
            on:click={() => handleSelectQuery(example.query)}
            class="inline-flex items-center px-2 py-1 rounded-md text-xs font-mono bg-gray-100 dark:bg-gray-700 text-blue-600 dark:text-blue-400 hover:bg-blue-100 dark:hover:bg-blue-900/30 hover:text-blue-700 dark:hover:text-blue-300 transition-colors cursor-pointer"
            title={$_(example.label)}
          >
            {example.query}
          </button>
        {/each}
      </div>
    </div>
  </div>
{/if}
