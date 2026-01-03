<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { ALL_LANGUAGES, CORE_LANGUAGES, calculateTotalSize } from '../../utils/syntaxLanguages';

  export let enabledSyntaxLanguages: string[];
</script>

<div class="space-y-6">
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
            on:click={() => enabledSyntaxLanguages = CORE_LANGUAGES.map(l => l.id)}
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
      <!-- Core Languages (Always Enabled) -->
      <div>
        <h4 class="text-sm font-medium text-gray-900 dark:text-white mb-2 flex items-center gap-2">
          {$_('settings.editorTab.coreLanguages')}
          <span class="text-xs text-gray-500 dark:text-gray-400">{$_('settings.editorTab.alwaysEnabled')}</span>
        </h4>
        <div class="grid grid-cols-1 tablet:grid-cols-2 gap-2">
          {#each CORE_LANGUAGES as lang}
            <div class="flex items-center gap-2 p-2 bg-gray-50 dark:bg-gray-700 rounded border border-gray-200 dark:border-gray-600">
              <input
                type="checkbox"
                checked={true}
                disabled={true}
                class="rounded border-gray-300 dark:border-gray-600 opacity-50"
              />
              <div class="flex-1">
                <span class="text-sm font-medium text-gray-900 dark:text-white">{lang.name}</span>
                {#if lang.aliases.length > 0}
                  <span class="text-xs text-gray-500 dark:text-gray-400 ml-1">
                    ({lang.aliases.slice(0, 2).join(', ')}{lang.aliases.length > 2 ? '...' : ''})
                  </span>
                {/if}
              </div>
              <span class="text-xs text-gray-500 dark:text-gray-400">{lang.estimatedSize} KB</span>
            </div>
          {/each}
        </div>
      </div>

      <!-- Popular Languages -->
      <div>
        <h4 class="text-sm font-medium text-gray-900 dark:text-white mb-2">{$_('settings.editorTab.popularLanguages')}</h4>
        <div class="grid grid-cols-1 tablet:grid-cols-2 gap-2">
          {#each ALL_LANGUAGES.filter(l => l.category === 'popular') as lang}
            <label class="flex items-center gap-2 p-2 bg-white dark:bg-gray-800 rounded border border-gray-200 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer">
              <input
                type="checkbox"
                checked={enabledSyntaxLanguages.includes(lang.id)}
                on:change={(e) => {
                  if (e.currentTarget.checked) {
                    enabledSyntaxLanguages = [...enabledSyntaxLanguages, lang.id];
                  } else {
                    enabledSyntaxLanguages = enabledSyntaxLanguages.filter(id => id !== lang.id);
                  }
                }}
                class="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
              />
              <div class="flex-1">
                <span class="text-sm font-medium text-gray-900 dark:text-white">{lang.name}</span>
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
                <label class="flex items-center gap-2 p-2 bg-white dark:bg-gray-800 rounded border border-gray-200 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={enabledSyntaxLanguages.includes(lang.id)}
                    on:change={(e) => {
                      if (e.currentTarget.checked) {
                        enabledSyntaxLanguages = [...enabledSyntaxLanguages, lang.id];
                      } else {
                        enabledSyntaxLanguages = enabledSyntaxLanguages.filter(id => id !== lang.id);
                      }
                    }}
                    class="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
                  />
                  <div class="flex-1">
                    <span class="text-sm font-medium text-gray-900 dark:text-white">{lang.name}</span>
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
