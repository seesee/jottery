<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { getColorHex, getTagColor } from '../services/colorService';
  import { isVirtualTag, isTitleTag, getTitleFromTag } from '../utils/virtualTags';
  import { localeIncludes } from '../utils/stringUtils';

  export let tags: string[] = [];
  export let onChange: (tags: string[]) => void = () => {};
  export let placeholder: string = 'Add tags...';
  export let onTagClick: ((tag: string) => void) | undefined = undefined;
  export let disabled: boolean = false;

  let inputValue = '';
  let suggestions: string[] = [];
  let showSuggestions = false;
  let selectedSuggestionIndex = -1;

  // All available tags from existing notes (would be passed as prop in real implementation)
  export let availableTags: string[] = [];

  // Track current theme based on DOM class
  let currentTheme: 'light' | 'dark' = 'light';
  let themeObserver: MutationObserver | null = null;

  onMount(() => {
    // Set up theme observer to watch for dark mode changes
    const updateTheme = () => {
      // Check actual DOM class instead of settings to handle forced themes
      currentTheme = document.documentElement.classList.contains('dark') ? 'dark' : 'light';
    };

    // Initial check
    updateTheme();

    // Watch for theme changes on document element
    themeObserver = new MutationObserver(updateTheme);
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['class'],
    });
  });

  onDestroy(() => {
    if (themeObserver) {
      themeObserver.disconnect();
      themeObserver = null;
    }
  });

  // Helper function to get tag color (reactive to re-run when currentTheme changes)
  $: getTagBackgroundColor = (tag: string): string | undefined => {
    const tagColorName = getTagColor(tag);
    return tagColorName ? getColorHex(tagColorName, currentTheme) : undefined;
  };

  function handleInput() {
    if (inputValue.trim()) {
      // Filter available tags for suggestions (exclude virtual tags from autocomplete)
      suggestions = availableTags
        .filter(tag => !isVirtualTag(tag) && !tags.includes(tag) && localeIncludes(tag, inputValue))
        .slice(0, 5);
      showSuggestions = suggestions.length > 0;
      selectedSuggestionIndex = -1;
    } else {
      showSuggestions = false;
      suggestions = [];
    }
  }

  function addTag(tag: string) {
    const trimmedTag = tag.trim();
    if (!trimmedTag) return;

    // Handle title tags specially - only one allowed, remove existing
    // Supports both t: and title: prefixes
    if (isTitleTag(trimmedTag)) {
      // Remove any existing title tags first (both t: and title: prefixes)
      const filtered = tags.filter(t => !isTitleTag(t));
      tags = [...filtered, trimmedTag];
      onChange(tags);
      inputValue = '';
      showSuggestions = false;
      suggestions = [];
      return;
    }

    // Regular tags - check for duplicates
    if (!tags.includes(trimmedTag)) {
      tags = [...tags, trimmedTag];
      onChange(tags);
      inputValue = '';
      showSuggestions = false;
      suggestions = [];
    }
  }

  function removeTag(index: number) {
    tags = tags.filter((_, i) => i !== index);
    onChange(tags);
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      if (selectedSuggestionIndex >= 0 && suggestions[selectedSuggestionIndex]) {
        addTag(suggestions[selectedSuggestionIndex]);
      } else if (inputValue.trim()) {
        addTag(inputValue);
      }
    } else if (e.key === 'Backspace' && !inputValue && tags.length > 0) {
      removeTag(tags.length - 1);
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (showSuggestions) {
        selectedSuggestionIndex = Math.min(selectedSuggestionIndex + 1, suggestions.length - 1);
      }
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (showSuggestions) {
        selectedSuggestionIndex = Math.max(selectedSuggestionIndex - 1, -1);
      }
    } else if (e.key === 'Escape') {
      showSuggestions = false;
      selectedSuggestionIndex = -1;
    }
  }

  function handleBlur() {
    // Delay to allow click on suggestion
    setTimeout(() => {
      showSuggestions = false;
      selectedSuggestionIndex = -1;
    }, 200);
  }
</script>

<div class="relative">
  <div class="tag-input-container flex flex-wrap gap-2 p-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-800 min-h-[2.5rem]">
    <!-- Existing tags -->
    {#each tags as tag, index}
      {@const tagBgColor = getTagBackgroundColor(tag)}
      {@const isTitle = isTitleTag(tag)}
      {@const titleValue = isTitle ? getTitleFromTag(tag) : null}
      <span
        class="tag-pill inline-flex items-center gap-1 px-2 py-1 text-sm rounded-md {isTitle ? 'italic bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400' : tagBgColor ? 'text-gray-900 dark:text-gray-100' : 'bg-gray-200 dark:bg-gray-600 text-gray-800 dark:text-gray-300'}"
        style:background-color={!isTitle ? tagBgColor : undefined}
      >
        <!-- svelte-ignore a11y-no-noninteractive-tabindex -->
        <span
          on:click={() => onTagClick?.(tag)}
          class="{onTagClick ? 'cursor-pointer hover:underline' : ''}"
          role={onTagClick ? 'button' : undefined}
          tabindex={onTagClick ? 0 : -1}
          on:keydown={(e) => {
            if (onTagClick && (e.key === 'Enter' || e.key === ' ')) {
              e.preventDefault();
              onTagClick(tag);
            }
          }}
        >
          {#if isTitle}
            {$_('tags.virtual.title')}: {titleValue}
          {:else}
            #{tag}
          {/if}
        </span>
        {#if !disabled}
          <button
            on:click={() => removeTag(index)}
            class="tag-remove hover:text-blue-600 dark:hover:text-blue-400"
            title={$_('tagInput.removeTag')}
          >
            ×
          </button>
        {/if}
      </span>
    {/each}

    <!-- Input -->
    {#if !disabled}
      <input
        type="text"
        bind:value={inputValue}
        on:input={handleInput}
        on:keydown={handleKeyDown}
        on:blur={handleBlur}
        {placeholder}
        enterkeyhint="done"
        autocapitalize="none"
        autocomplete="off"
        role="combobox"
        aria-autocomplete="list"
        aria-expanded={showSuggestions}
        aria-haspopup="listbox"
        aria-controls={showSuggestions ? 'tag-input-suggestions' : undefined}
        aria-activedescendant={showSuggestions && selectedSuggestionIndex >= 0 ? `tag-input-suggestion-${selectedSuggestionIndex}` : undefined}
        class="flex-1 min-w-[120px] outline-none bg-transparent text-sm text-gray-900 dark:text-gray-100"
      />
    {/if}
  </div>

  <!-- Suggestions dropdown -->
  {#if showSuggestions && !disabled}
    <div
      id="tag-input-suggestions"
      role="listbox"
      aria-label="Tag suggestions"
      class="absolute z-10 w-full mt-1 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-md shadow-lg max-h-48 overflow-y-auto"
    >
      {#each suggestions as suggestion, index}
        <div
          id="tag-input-suggestion-{index}"
          role="option"
          aria-selected={index === selectedSuggestionIndex}
          on:click={() => addTag(suggestion)}
          on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && addTag(suggestion)}
          tabindex="-1"
          class="w-full text-left px-3 py-2 text-sm cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700 {index === selectedSuggestionIndex ? 'bg-gray-100 dark:bg-gray-700' : ''}"
        >
          #{suggestion}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  /* Prevent layout shift when suggestions appear */
  .relative {
    position: relative;
  }

  /* Mobile improvements for better touch targets and spacing */
  @media (max-width: 768px) {
    .tag-input-container {
      padding: 8px 12px;
      gap: 8px;
      min-height: 44px;
    }

    .tag-pill {
      padding: 8px 12px;
      gap: 8px;
      min-height: 36px;
    }

    .tag-remove {
      padding: 4px;
      min-width: 24px;
      min-height: 24px;
      font-size: 18px;
      display: flex;
      align-items: center;
      justify-content: center;
    }
  }
</style>
