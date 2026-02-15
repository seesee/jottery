<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { onMount } from 'svelte';
  import type { ColorPalette } from '../../types/models';
  import { DEFAULT_COLOR_PALETTE } from '../../types/models';
  import { tagService } from '../../services';
  import { settings, notes } from '../../stores/appStore';
  import { resolveTheme, getColorDisplayName } from '../../services/colorService';
  import { localeIncludes, localeSort } from '../../utils/stringUtils';

  export let colorPalette: ColorPalette;
  export let tagColors: Record<string, string>;

  let allTags: string[] = [];
  let newTagName = '';
  let newTagColor = '';
  let tagSuggestions: string[] = [];
  let showTagSuggestions = false;
  let selectedTagIndex = -1;

  $: currentTheme = resolveTheme($settings.theme);

  onMount(async () => {
    // Load all existing tags from the notes store and normalize to lowercase
    allTags = tagService.getAllTags($notes).map(tag => tag.toLowerCase());
  });

  function resetPalette() {
    colorPalette = { ...DEFAULT_COLOR_PALETTE };
  }

  function handleTagInput() {
    if (newTagName.trim()) {
      // Filter available tags for suggestions (case-insensitive)
      tagSuggestions = unassignedTags
        .filter(tag => localeIncludes(tag, newTagName.trim()))
        .slice(0, 8); // Show up to 8 suggestions
      showTagSuggestions = tagSuggestions.length > 0;
      selectedTagIndex = -1;
    } else {
      showTagSuggestions = false;
      tagSuggestions = [];
    }
  }

  function selectTag(tag: string) {
    newTagName = tag;
    showTagSuggestions = false;
    tagSuggestions = [];
    selectedTagIndex = -1;
  }

  function handleTagKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      event.preventDefault();
      if (selectedTagIndex >= 0 && tagSuggestions[selectedTagIndex]) {
        selectTag(tagSuggestions[selectedTagIndex]);
      }
    } else if (event.key === 'ArrowDown') {
      event.preventDefault();
      if (showTagSuggestions) {
        selectedTagIndex = Math.min(selectedTagIndex + 1, tagSuggestions.length - 1);
      }
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      if (showTagSuggestions) {
        selectedTagIndex = Math.max(selectedTagIndex - 1, -1);
      }
    } else if (event.key === 'Escape') {
      showTagSuggestions = false;
      selectedTagIndex = -1;
    }
  }

  function handleTagBlur() {
    // Delay to allow click on suggestion
    setTimeout(() => {
      showTagSuggestions = false;
      selectedTagIndex = -1;
    }, 200);
  }

  function addTagColor() {
    if (newTagName && newTagColor) {
      // Normalize tag name to lowercase for case-insensitive storage
      const normalizedTag = newTagName.toLowerCase().trim();
      tagColors = { ...tagColors, [normalizedTag]: newTagColor };
      newTagName = '';
      newTagColor = '';
    }
  }

  function removeTagColor(tag: string) {
    const updated = { ...tagColors };
    delete updated[tag];
    tagColors = updated;
  }

  // Get available color names for dropdowns
  $: colorOptions = Object.keys(colorPalette);

  // Get unassigned tags (tags that exist but don't have colors)
  // allTags are already normalized to lowercase
  $: unassignedTags = allTags.filter(tag => !tagColors[tag]);

  // Sort tag colors alphabetically
  $: sortedTagColors = Object.entries(tagColors).sort(([a], [b]) => localeSort(a, b));
</script>

<!-- Color Palette Section -->
<div class="mb-8">
  <h3 class="text-lg font-medium text-gray-900 dark:text-gray-100 mb-2">
    {$_('settings.colors.paletteTitle')}
  </h3>
  <p class="text-sm text-gray-600 dark:text-gray-400 mb-4">
    {$_('settings.colors.paletteDescription')}
  </p>

  <div class="space-y-4">
    {#each Object.entries(colorPalette) as [name, colors]}
      <div class="border border-gray-200 dark:border-gray-700 rounded-lg p-3">
        <div class="flex items-center gap-3 mb-2">
          <span class="text-xs text-gray-500 dark:text-gray-400 uppercase w-16 flex-shrink-0">
            {name}
          </span>
          <input
            type="text"
            bind:value={colors.displayName}
            placeholder="{name}"
            class="flex-1 px-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-700 text-gray-900 dark:text-white placeholder-gray-400 dark:placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
            aria-label="Display name for {name}"
          />
        </div>
        <div class="flex items-center gap-2">
          <div class="flex items-center gap-2">
            <label for="color-{name}-light" class="text-xs text-gray-600 dark:text-gray-400">
              {$_('settings.colors.lightMode')}
            </label>
            <input
              id="color-{name}-light"
              type="color"
              bind:value={colors.light}
              class="w-12 h-8 rounded border border-gray-300 dark:border-gray-600 cursor-pointer"
              aria-label="{name} light mode color"
            />
          </div>
          <div class="flex items-center gap-2">
            <label for="color-{name}-dark" class="text-xs text-gray-600 dark:text-gray-400">
              {$_('settings.colors.darkMode')}
            </label>
            <input
              id="color-{name}-dark"
              type="color"
              bind:value={colors.dark}
              class="w-12 h-8 rounded border border-gray-300 dark:border-gray-600 cursor-pointer"
              aria-label="{name} dark mode color"
            />
          </div>
          <div
            class="w-20 h-8 rounded border border-gray-300 dark:border-gray-600 ml-auto"
            style="background-color: {currentTheme === 'light' ? colors.light : colors.dark}"
            title="Preview ({currentTheme} mode)"
          ></div>
        </div>
      </div>
    {/each}
  </div>

  <button
    on:click={resetPalette}
    class="mt-4 px-4 py-2 bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600 text-gray-900 dark:text-gray-100 rounded-md transition-colors"
  >
    {$_('settings.colors.resetPalette')}
  </button>
</div>

<!-- Tag Colors Section -->
<div>
  <h3 class="text-lg font-medium text-gray-900 dark:text-gray-100 mb-2">
    {$_('settings.colors.tagColorsTitle')}
  </h3>
  <p class="text-sm text-gray-600 dark:text-gray-400 mb-4">
    {$_('settings.colors.tagColorsDescription')}
  </p>

  <!-- Existing tag colors -->
  {#if sortedTagColors.length > 0}
    <div class="space-y-2 mb-4">
      {#each sortedTagColors as [tag, color]}
        <div class="flex items-center gap-3">
          <span class="flex-1 text-sm text-gray-700 dark:text-gray-300">
            #{tag}
          </span>
          <select
            bind:value={tagColors[tag]}
            class="px-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
          >
            {#each colorOptions as colorName}
              <option value={colorName}>{getColorDisplayName(colorName)}</option>
            {/each}
          </select>
          <div
            class="w-8 h-8 rounded border border-gray-300 dark:border-gray-600"
            style="background-color: {currentTheme === 'light' ? colorPalette[color]?.light : colorPalette[color]?.dark}"
            title="{color}"
          ></div>
          <button
            on:click={() => removeTagColor(tag)}
            class="px-3 py-1.5 text-sm text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 rounded transition-colors"
          >
            {$_('settings.colors.removeTagColor')}
          </button>
        </div>
      {/each}
    </div>
  {:else}
    <p class="text-sm text-gray-500 dark:text-gray-400 italic mb-4">
      {$_('settings.colors.noTagColors')}
    </p>
  {/if}

  <!-- Add new tag color -->
  <div class="border-t border-gray-200 dark:border-gray-700 pt-4">
    <p class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
      {$_('settings.colors.addTagColor')}
    </p>
    <div class="flex items-center gap-2">
      <!-- Tag name input with autocomplete -->
      <div class="flex-1 relative">
        <input
          type="text"
          bind:value={newTagName}
          on:input={handleTagInput}
          on:keydown={handleTagKeydown}
          on:blur={handleTagBlur}
          placeholder={$_('settings.colors.tagNamePlaceholder')}
          autocomplete="off"
          autocapitalize="none"
          class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-700 text-gray-900 dark:text-white placeholder-gray-400 dark:placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
        />

        <!-- Autocomplete suggestions -->
        {#if showTagSuggestions}
          <div class="absolute z-10 w-full mt-1 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-md shadow-lg max-h-48 overflow-y-auto">
            {#each tagSuggestions as suggestion, index}
              <button
                type="button"
                on:click={() => selectTag(suggestion)}
                class="w-full text-left px-3 py-2 text-sm hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors {index === selectedTagIndex ? 'bg-gray-100 dark:bg-gray-700' : ''}"
              >
                #{suggestion}
              </button>
            {/each}
          </div>
        {/if}
      </div>

      <!-- Color selection -->
      <select
        bind:value={newTagColor}
        class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
      >
        <option value="">{$_('settings.colors.selectColor')}</option>
        {#each colorOptions as colorName}
          <option value={colorName}>{getColorDisplayName(colorName)}</option>
        {/each}
      </select>

      <!-- Add button -->
      <button
        on:click={addTagColor}
        disabled={!newTagName || !newTagColor}
        class="px-4 py-2 bg-blue-500 hover:bg-blue-600 disabled:bg-gray-300 dark:disabled:bg-gray-600 disabled:cursor-not-allowed text-white rounded transition-colors"
      >
        {$_('common.add')}
      </button>
    </div>
  </div>
</div>
