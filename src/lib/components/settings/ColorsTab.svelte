<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { onMount } from 'svelte';
  import type { ColorPalette } from '../../types/models';
  import { DEFAULT_COLOR_PALETTE, COLOR_NAMES } from '../../types/models';
  import { tagService } from '../../services';
  import { settings } from '../../stores/appStore';
  import { resolveTheme } from '../../services/colorService';

  export let colorPalette: ColorPalette;
  export let tagColors: Record<string, string>;

  let allTags: string[] = [];
  let newTagName = '';
  let newTagColor = '';

  $: currentTheme = resolveTheme($settings.theme);

  onMount(async () => {
    // Load all existing tags
    allTags = await tagService.getAllTags();
  });

  function resetPalette() {
    colorPalette = { ...DEFAULT_COLOR_PALETTE };
  }

  function addTagColor() {
    if (newTagName && newTagColor) {
      tagColors = { ...tagColors, [newTagName]: newTagColor };
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
  $: unassignedTags = allTags.filter(tag => !tagColors[tag]);

  // Sort tag colors alphabetically
  $: sortedTagColors = Object.entries(tagColors).sort(([a], [b]) => a.localeCompare(b));
</script>

<!-- Color Palette Section -->
<div class="mb-8">
  <h3 class="text-lg font-medium text-gray-900 dark:text-gray-100 mb-2">
    {$_('settings.colors.paletteTitle')}
  </h3>
  <p class="text-sm text-gray-600 dark:text-gray-400 mb-4">
    {$_('settings.colors.paletteDescription')}
  </p>

  <div class="space-y-3">
    {#each Object.entries(colorPalette) as [name, colors]}
      <div class="flex items-center gap-3">
        <span class="w-20 text-sm font-medium text-gray-700 dark:text-gray-300 capitalize">
          {name}
        </span>
        <div class="flex items-center gap-2 flex-1">
          <div class="flex items-center gap-2">
            <label class="text-xs text-gray-600 dark:text-gray-400">
              {$_('settings.colors.lightMode')}
            </label>
            <input
              type="color"
              bind:value={colors.light}
              class="w-12 h-8 rounded border border-gray-300 dark:border-gray-600 cursor-pointer"
              aria-label="{name} light mode color"
            />
          </div>
          <div class="flex items-center gap-2">
            <label class="text-xs text-gray-600 dark:text-gray-400">
              {$_('settings.colors.darkMode')}
            </label>
            <input
              type="color"
              bind:value={colors.dark}
              class="w-12 h-8 rounded border border-gray-300 dark:border-gray-600 cursor-pointer"
              aria-label="{name} dark mode color"
            />
          </div>
        </div>
        <div
          class="w-20 h-8 rounded border border-gray-300 dark:border-gray-600"
          style="background-color: {currentTheme === 'light' ? colors.light : colors.dark}"
          title="Preview ({currentTheme} mode)"
        ></div>
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
              <option value={colorName}>{colorName}</option>
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
      <div class="flex-1">
        {#if unassignedTags.length > 0}
          <select
            bind:value={newTagName}
            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
          >
            <option value="">{$_('settings.colors.selectTag')}</option>
            {#each unassignedTags as tag}
              <option value={tag}>#{tag}</option>
            {/each}
          </select>
        {:else}
          <input
            type="text"
            bind:value={newTagName}
            placeholder={$_('settings.colors.tagNamePlaceholder')}
            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-700 text-gray-900 dark:text-white placeholder-gray-400 dark:placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        {/if}
      </div>

      <!-- Color selection -->
      <select
        bind:value={newTagColor}
        class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
      >
        <option value="">{$_('settings.colors.selectColor')}</option>
        {#each colorOptions as colorName}
          <option value={colorName}>{colorName}</option>
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
