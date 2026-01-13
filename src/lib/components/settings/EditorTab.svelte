<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { ALL_LANGUAGES, CORE_LANGUAGES, calculateTotalSize } from '../../utils/syntaxLanguages';

  export let enabledSyntaxLanguages: string[];
  export let defaultSyntaxLanguage: string;
  export let vimMode: boolean;
  export let quickCommands: boolean;

  // Ensure default language is always in enabled list
  $: if (defaultSyntaxLanguage && !enabledSyntaxLanguages.includes(defaultSyntaxLanguage)) {
    enabledSyntaxLanguages = [...enabledSyntaxLanguages, defaultSyntaxLanguage];
  }

  // Get only enabled languages for the default selector
  $: enabledLanguageObjects = ALL_LANGUAGES.filter(l => enabledSyntaxLanguages.includes(l.id));

  // Handle language toggle with default protection
  function toggleLanguage(langId: string, enabled: boolean) {
    if (enabled) {
      enabledSyntaxLanguages = [...enabledSyntaxLanguages, langId];
    } else {
      // Prevent deselecting the default language
      if (langId === defaultSyntaxLanguage) {
        return;
      }
      enabledSyntaxLanguages = enabledSyntaxLanguages.filter(id => id !== langId);
    }
  }
</script>

<div class="space-y-6">
  <!-- Vim Mode -->
  <div>
    <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-2">{$_('settings.editorTab.vimMode')}</h3>
    <p class="text-sm text-gray-600 dark:text-gray-400 mb-4">
      {$_('settings.editorTab.vimModeHelp')}
    </p>
    <label class="flex items-center gap-3 cursor-pointer">
      <input
        type="checkbox"
        bind:checked={vimMode}
        class="w-5 h-5 rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
      />
      <span class="text-sm font-medium text-gray-900 dark:text-white">
        {$_('settings.editorTab.enableVimMode')}
      </span>
    </label>
  </div>

  <!-- Quick Commands -->
  <div>
    <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-2">{$_('settings.editorTab.quickCommands')}</h3>
    <p class="text-sm text-gray-600 dark:text-gray-400 mb-4">
      {$_('settings.editorTab.quickCommandsHelp')}
    </p>
    <label class="flex items-center gap-3 cursor-pointer">
      <input
        type="checkbox"
        bind:checked={quickCommands}
        class="w-5 h-5 rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
      />
      <span class="text-sm font-medium text-gray-900 dark:text-white">
        {$_('settings.editorTab.enableQuickCommands')}
      </span>
    </label>
  </div>

  <!-- Default Note Type -->
  <div>
    <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-2">{$_('settings.editorTab.defaultNoteType')}</h3>
    <p class="text-sm text-gray-600 dark:text-gray-400 mb-4">
      {$_('settings.editorTab.defaultNoteTypeHelp')}
    </p>
    <select
      bind:value={defaultSyntaxLanguage}
      class="w-full tablet:w-auto px-3 py-2 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500"
    >
      {#each enabledLanguageObjects as lang}
        <option value={lang.id}>{lang.name}</option>
      {/each}
    </select>
  </div>

  <!-- Syntax Highlighting Languages -->
  <div>
    <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-4">{$_('settings.editorTab.syntaxHighlighting')}</h3>
    <p class="text-sm text-gray-600 dark:text-gray-400 mb-4">
      {$_('settings.editorTab.syntaxHelp')}
    </p>

    <!-- Summary Stats -->
    <div class="bg-gray-50 dark:bg-gray-700 rounded-lg p-4 mb-4">
      <div class="flex items-center justify-between">
        <div>
          <p class="text-sm font-medium text-gray-900 dark:text-white">
            {$_('settings.editorTab.languagesEnabled', { values: { count: enabledSyntaxLanguages.length } })}
          </p>
          <p class="text-xs text-gray-500 dark:text-gray-400">
            {$_('settings.editorTab.estimatedSize', { values: { size: calculateTotalSize(enabledSyntaxLanguages) } })}
          </p>
        </div>
        <div class="flex gap-2">
          <button
            on:click={() => {
              enabledSyntaxLanguages = CORE_LANGUAGES.map(l => l.id);
              // Ensure default is still enabled
              if (!enabledSyntaxLanguages.includes(defaultSyntaxLanguage)) {
                enabledSyntaxLanguages = [...enabledSyntaxLanguages, defaultSyntaxLanguage];
              }
            }}
            class="px-3 py-1 text-xs font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-600 border border-gray-300 dark:border-gray-500 rounded hover:bg-gray-50 dark:hover:bg-gray-500"
          >
            {$_('settings.editorTab.resetToCore')}
          </button>
          <button
            on:click={() => enabledSyntaxLanguages = ALL_LANGUAGES.map(l => l.id)}
            class="px-3 py-1 text-xs font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-600 border border-gray-300 dark:border-gray-500 rounded hover:bg-gray-50 dark:hover:bg-gray-500"
          >
            {$_('settings.editorTab.enableAll')}
          </button>
        </div>
      </div>
    </div>

    <!-- Language Groups -->
    <div class="space-y-4">
      <!-- Core Languages -->
      <div>
        <h4 class="text-sm font-medium text-gray-900 dark:text-white mb-2">
          {$_('settings.editorTab.coreLanguages')}
        </h4>
        <div class="grid grid-cols-1 tablet:grid-cols-2 gap-2">
          {#each CORE_LANGUAGES as lang}
            {@const isDefault = lang.id === defaultSyntaxLanguage}
            {@const isEnabled = enabledSyntaxLanguages.includes(lang.id)}
            <label
              class="flex items-center gap-2 p-2 rounded border cursor-pointer
                {isDefault ? 'bg-blue-50 dark:bg-blue-900/20 border-blue-300 dark:border-blue-700' : 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700'}"
            >
              <input
                type="checkbox"
                checked={isEnabled}
                disabled={isDefault}
                on:change={(e) => toggleLanguage(lang.id, e.currentTarget.checked)}
                class="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500 {isDefault ? 'opacity-50' : ''}"
              />
              <div class="flex-1">
                <span class="text-sm font-medium text-gray-900 dark:text-white">
                  {lang.name}
                  {#if isDefault}
                    <span class="text-xs text-blue-600 dark:text-blue-400 ml-1">({$_('settings.editorTab.default')})</span>
                  {/if}
                </span>
                {#if lang.aliases.length > 0}
                  <span class="text-xs text-gray-500 dark:text-gray-400 ml-1">
                    ({lang.aliases.slice(0, 2).join(', ')}{lang.aliases.length > 2 ? '...' : ''})
                  </span>
                {/if}
              </div>
              <span class="text-xs text-gray-500 dark:text-gray-400">{lang.estimatedSize} KB</span>
            </label>
          {/each}
        </div>
      </div>

      <!-- Other Language Categories -->
      {#each [
        { id: 'web', key: 'settings.editorTab.webLanguages' },
        { id: 'systems', key: 'settings.editorTab.systemsLanguages' },
        { id: 'data', key: 'settings.editorTab.dataLanguages' },
        { id: 'other', key: 'settings.editorTab.otherLanguages' }
      ] as category}
        {@const categoryLangs = ALL_LANGUAGES.filter(l => l.category === category.id)}
        {#if categoryLangs.length > 0}
          <details class="group">
            <summary class="cursor-pointer list-none">
              <div class="flex items-center gap-2 text-sm font-medium text-gray-900 dark:text-white mb-2">
                <span class="group-open:rotate-90 transition-transform">▶</span>
                <span>{$_(category.key)}</span>
                <span class="text-xs text-gray-500 dark:text-gray-400">
                  {$_('settings.editorTab.enabledCount', { values: { enabled: categoryLangs.filter(l => enabledSyntaxLanguages.includes(l.id)).length, total: categoryLangs.length } })}
                </span>
              </div>
            </summary>
            <div class="grid grid-cols-1 tablet:grid-cols-2 gap-2 mt-2">
              {#each categoryLangs as lang}
                {@const isDefault = lang.id === defaultSyntaxLanguage}
                {@const isEnabled = enabledSyntaxLanguages.includes(lang.id)}
                <label
                  class="flex items-center gap-2 p-2 rounded border cursor-pointer
                    {isDefault ? 'bg-blue-50 dark:bg-blue-900/20 border-blue-300 dark:border-blue-700' : 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700'}"
                >
                  <input
                    type="checkbox"
                    checked={isEnabled}
                    disabled={isDefault}
                    on:change={(e) => toggleLanguage(lang.id, e.currentTarget.checked)}
                    class="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500 {isDefault ? 'opacity-50' : ''}"
                  />
                  <div class="flex-1">
                    <span class="text-sm font-medium text-gray-900 dark:text-white">
                      {lang.name}
                      {#if isDefault}
                        <span class="text-xs text-blue-600 dark:text-blue-400 ml-1">({$_('settings.editorTab.default')})</span>
                      {/if}
                    </span>
                    {#if lang.aliases.length > 0}
                      <span class="text-xs text-gray-500 dark:text-gray-400 ml-1">
                        ({lang.aliases.slice(0, 2).join(', ')}{lang.aliases.length > 2 ? '...' : ''})
                      </span>
                    {/if}
                  </div>
                  <span class="text-xs text-gray-500 dark:text-gray-400">{lang.estimatedSize} KB</span>
                </label>
              {/each}
            </div>
          </details>
        {/if}
      {/each}
    </div>
  </div>
</div>
