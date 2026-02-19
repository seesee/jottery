<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { onMount, onDestroy } from 'svelte';

  export let show = false;
  export let onClose: () => void;
  export let onInsertExample: (query: string) => void;

  let popoverEl: HTMLElement;

  function handleClickOutside(event: MouseEvent) {
    if (popoverEl && !popoverEl.contains(event.target as Node)) {
      onClose();
    }
  }

  function handleKeyDown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      onClose();
    }
  }

  function handleExampleClick(query: string) {
    onInsertExample(query);
  }

  onMount(() => {
    document.addEventListener('mousedown', handleClickOutside);
    document.addEventListener('keydown', handleKeyDown);
  });

  onDestroy(() => {
    document.removeEventListener('mousedown', handleClickOutside);
    document.removeEventListener('keydown', handleKeyDown);
  });
</script>

{#if show}
  <!-- Backdrop for mobile to prevent interaction with elements below -->
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 bg-black/20 z-50 sm:hidden"
    on:click={onClose}
  ></div>
  <div
    bind:this={popoverEl}
    class="fixed inset-x-4 top-16 z-50 mx-auto max-w-96 sm:absolute sm:inset-auto sm:right-0 sm:top-full sm:mt-2 sm:mx-0 sm:w-96 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-600 rounded-lg shadow-xl max-h-[70vh] overflow-y-auto"
    role="dialog"
    aria-label={$_('searchHelp.title')}
  >
    <!-- Header -->
    <div class="sticky top-0 bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 px-4 py-3 flex items-center justify-between">
      <h3 class="text-sm font-bold text-gray-900 dark:text-white">{$_('searchHelp.title')}</h3>
      <button
        on:click={onClose}
        class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 transition-colors"
        aria-label={$_('common.close')}
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </div>

    <div class="px-4 py-3 space-y-4">
      <!-- Hint -->
      <p class="text-xs text-gray-500 dark:text-gray-400 italic">{$_('searchHelp.clickToTry')}</p>

      <!-- Tags -->
      <div>
        <h4 class="text-xs font-semibold text-gray-700 dark:text-gray-300 uppercase tracking-wide mb-1.5">{$_('searchHelp.tags')}</h4>
        <div class="space-y-1">
          <div class="flex items-baseline gap-2">
            <button
              on:click={() => handleExampleClick('#tagname')}
              class="font-mono text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer shrink-0"
            >#tagname</button>
            <span class="text-xs text-gray-500 dark:text-gray-400">&mdash; {$_('searchHelp.tags')}</span>
          </div>
          <div class="flex items-baseline gap-2">
            <button
              on:click={() => handleExampleClick('#tag1 #tag2')}
              class="font-mono text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer shrink-0"
            >#tag1 #tag2</button>
            <span class="text-xs text-gray-500 dark:text-gray-400">&mdash; {$_('searchHelp.tagsAnd')}</span>
          </div>
          <div class="flex items-baseline gap-2">
            <button
              on:click={() => handleExampleClick('#tag1 | #tag2')}
              class="font-mono text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer shrink-0"
            >#tag1 | #tag2</button>
            <span class="text-xs text-gray-500 dark:text-gray-400">&mdash; {$_('searchHelp.tagsOr')}</span>
          </div>
        </div>
      </div>

      <!-- Text -->
      <div>
        <h4 class="text-xs font-semibold text-gray-700 dark:text-gray-300 uppercase tracking-wide mb-1.5">{$_('searchHelp.text')}</h4>
        <div class="space-y-1">
          <div class="flex items-baseline gap-2">
            <button
              on:click={() => handleExampleClick('word')}
              class="font-mono text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer shrink-0"
            >word</button>
            <span class="text-xs text-gray-500 dark:text-gray-400">&mdash; {$_('searchHelp.text')}</span>
          </div>
          <div class="flex items-baseline gap-2">
            <button
              on:click={() => handleExampleClick('"exact phrase"')}
              class="font-mono text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer shrink-0"
            >"exact phrase"</button>
            <span class="text-xs text-gray-500 dark:text-gray-400">&mdash; {$_('searchHelp.exactPhrase')}</span>
          </div>
          <div class="flex items-baseline gap-2">
            <button
              on:click={() => handleExampleClick('word1 word2')}
              class="font-mono text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer shrink-0"
            >word1 word2</button>
            <span class="text-xs text-gray-500 dark:text-gray-400">&mdash; {$_('searchHelp.bothWords')}</span>
          </div>
          <div class="flex items-baseline gap-2">
            <button
              on:click={() => handleExampleClick('word1 | word2')}
              class="font-mono text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer shrink-0"
            >word1 | word2</button>
            <span class="text-xs text-gray-500 dark:text-gray-400">&mdash; {$_('searchHelp.eitherWord')}</span>
          </div>
          <div class="flex items-baseline gap-2">
            <button
              on:click={() => handleExampleClick('-word')}
              class="font-mono text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer shrink-0"
            >-word</button>
            <span class="text-xs text-gray-500 dark:text-gray-400">&mdash; {$_('searchHelp.notContaining')}</span>
          </div>
        </div>
      </div>

      <!-- Wildcards -->
      <div>
        <h4 class="text-xs font-semibold text-gray-700 dark:text-gray-300 uppercase tracking-wide mb-1.5">{$_('searchHelp.wildcards')}</h4>
        <div class="space-y-1">
          <div class="flex items-baseline gap-2">
            <button
              on:click={() => handleExampleClick('dog*')}
              class="font-mono text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer shrink-0"
            >dog*</button>
            <span class="text-xs text-gray-500 dark:text-gray-400">&mdash; {$_('searchHelp.startsWith')}</span>
          </div>
          <div class="flex items-baseline gap-2">
            <button
              on:click={() => handleExampleClick('*dog')}
              class="font-mono text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer shrink-0"
            >*dog</button>
            <span class="text-xs text-gray-500 dark:text-gray-400">&mdash; {$_('searchHelp.endsWith')}</span>
          </div>
          <div class="flex items-baseline gap-2">
            <button
              on:click={() => handleExampleClick('*dog*')}
              class="font-mono text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer shrink-0"
            >*dog*</button>
            <span class="text-xs text-gray-500 dark:text-gray-400">&mdash; {$_('searchHelp.containsAnywhere')}</span>
          </div>
        </div>
      </div>

      <!-- Modifiers -->
      <div>
        <h4 class="text-xs font-semibold text-gray-700 dark:text-gray-300 uppercase tracking-wide mb-1.5">{$_('searchHelp.modifiers')}</h4>
        <div class="space-y-1">
          <div class="flex items-baseline gap-2">
            <button
              on:click={() => handleExampleClick('has:attachment')}
              class="font-mono text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer shrink-0"
            >has:attachment</button>
            <span class="text-xs text-gray-500 dark:text-gray-400">&mdash; {$_('searchHelp.hasAttachment')}</span>
          </div>
          <div class="flex items-baseline gap-2">
            <button
              on:click={() => handleExampleClick('created:>2024-01-01')}
              class="font-mono text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer shrink-0"
            >created:&gt;2024-01-01</button>
            <span class="text-xs text-gray-500 dark:text-gray-400">&mdash; {$_('searchHelp.createdAfter')}</span>
          </div>
          <div class="flex items-baseline gap-2">
            <button
              on:click={() => handleExampleClick('created:<2024-06-30')}
              class="font-mono text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer shrink-0"
            >created:&lt;2024-06-30</button>
            <span class="text-xs text-gray-500 dark:text-gray-400">&mdash; {$_('searchHelp.createdBefore')}</span>
          </div>
          <div class="flex items-baseline gap-2">
            <button
              on:click={() => handleExampleClick('created:2024-01-01..2024-06-30')}
              class="font-mono text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer shrink-0"
            >created:2024-01-01..2024-06-30</button>
            <span class="text-xs text-gray-500 dark:text-gray-400">&mdash; {$_('searchHelp.createdRange')}</span>
          </div>
          <div class="flex items-baseline gap-2">
            <button
              on:click={() => handleExampleClick('modified:>2024-01-01')}
              class="font-mono text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer shrink-0"
            >modified:&gt;2024-01-01</button>
            <span class="text-xs text-gray-500 dark:text-gray-400">&mdash; {$_('searchHelp.modifiedAfter')}</span>
          </div>
          <div class="flex items-baseline gap-2">
            <button
              on:click={() => handleExampleClick('words:>100')}
              class="font-mono text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer shrink-0"
            >words:&gt;100</button>
            <span class="text-xs text-gray-500 dark:text-gray-400">&mdash; {$_('searchHelp.wordsMore')}</span>
          </div>
          <div class="flex items-baseline gap-2">
            <button
              on:click={() => handleExampleClick('words:<50')}
              class="font-mono text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer shrink-0"
            >words:&lt;50</button>
            <span class="text-xs text-gray-500 dark:text-gray-400">&mdash; {$_('searchHelp.wordsLess')}</span>
          </div>
          <div class="flex items-baseline gap-2">
            <button
              on:click={() => handleExampleClick('words:50..200')}
              class="font-mono text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer shrink-0"
            >words:50..200</button>
            <span class="text-xs text-gray-500 dark:text-gray-400">&mdash; {$_('searchHelp.wordsRange')}</span>
          </div>
        </div>
      </div>

      <!-- Combined Examples -->
      <div>
        <h4 class="text-xs font-semibold text-gray-700 dark:text-gray-300 uppercase tracking-wide mb-1.5">{$_('searchHelp.combined')}</h4>
        <div class="space-y-1">
          <div class="flex items-baseline gap-2">
            <button
              on:click={() => handleExampleClick('#animals dog -cat')}
              class="font-mono text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer shrink-0"
            >#animals dog -cat</button>
          </div>
          <div class="flex items-baseline gap-2">
            <button
              on:click={() => handleExampleClick('has:attachment created:>2024-01-01')}
              class="font-mono text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer shrink-0"
            >has:attachment created:&gt;2024-01-01</button>
          </div>
        </div>
      </div>
    </div>
  </div>
{/if}
