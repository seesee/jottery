<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { ALL_LANGUAGES, CORE_LANGUAGES, calculateTotalSize } from '../../utils/syntaxLanguages';
  import type { QuickCommandConfig } from '../../types/models';
  import { DEFAULT_QUICK_COMMANDS } from '../../types/models';
  import { generateCommandContent } from '../../extensions/quickCommands';

  export let enabledSyntaxLanguages: string[];
  export let defaultSyntaxLanguage: string;
  export let vimMode: boolean;
  export let quickCommandsEnabled: boolean;
  export let quickCommandsList: QuickCommandConfig[];

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
      if (langId === defaultSyntaxLanguage) {
        return;
      }
      enabledSyntaxLanguages = enabledSyntaxLanguages.filter(id => id !== langId);
    }
  }

  // Quick command editing state
  let editingCommand: QuickCommandConfig | null = null;
  let isNewCommand = false;

  // Command types for the dropdown
  const commandTypes: { value: QuickCommandConfig['type']; label: string }[] = [
    { value: 'datetime', label: 'Date & Time' },
    { value: 'date', label: 'Date' },
    { value: 'time', label: 'Time' },
    { value: 'uuid', label: 'UUID' },
    { value: 'text', label: 'Text' },
  ];

  function startAddCommand() {
    editingCommand = {
      id: crypto.randomUUID(),
      trigger: '/',
      type: 'text',
      value: '',
      description: '',
    };
    isNewCommand = true;
  }

  function startEditCommand(cmd: QuickCommandConfig) {
    editingCommand = { ...cmd };
    isNewCommand = false;
  }

  function cancelEdit() {
    editingCommand = null;
    isNewCommand = false;
  }

  function saveCommand() {
    if (!editingCommand) return;

    // Validate trigger
    if (!editingCommand.trigger.startsWith('/')) {
      editingCommand.trigger = '/' + editingCommand.trigger;
    }

    if (isNewCommand) {
      quickCommandsList = [...quickCommandsList, editingCommand];
    } else {
      quickCommandsList = quickCommandsList.map(cmd =>
        cmd.id === editingCommand!.id ? editingCommand! : cmd
      );
    }

    editingCommand = null;
    isNewCommand = false;
  }

  function deleteCommand(id: string) {
    quickCommandsList = quickCommandsList.filter(cmd => cmd.id !== id);
  }

  function resetToDefaults() {
    quickCommandsList = [...DEFAULT_QUICK_COMMANDS];
  }

  // Preview the output of a command
  function getPreview(cmd: QuickCommandConfig): string {
    try {
      return generateCommandContent(cmd);
    } catch {
      return '(error)';
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

    <!-- Enable toggle -->
    <label class="flex items-center gap-3 cursor-pointer mb-4">
      <input
        type="checkbox"
        bind:checked={quickCommandsEnabled}
        class="w-5 h-5 rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
      />
      <span class="text-sm font-medium text-gray-900 dark:text-white">
        {$_('settings.editorTab.enableQuickCommands')}
      </span>
    </label>

    {#if quickCommandsEnabled}
      <!-- Command list -->
      <div class="bg-gray-50 dark:bg-gray-700 rounded-lg p-4 mb-4">
        <div class="flex items-center justify-between mb-3">
          <span class="text-sm font-medium text-gray-900 dark:text-white">
            {$_('settings.editorTab.commandsConfigured', { values: { count: quickCommandsList.length } })}
          </span>
          <div class="flex gap-2">
            <button
              type="button"
              on:click={resetToDefaults}
              class="px-3 py-1 text-xs font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-600 border border-gray-300 dark:border-gray-500 rounded hover:bg-gray-50 dark:hover:bg-gray-500"
            >
              {$_('settings.editorTab.resetToDefaults')}
            </button>
            <button
              type="button"
              on:click={startAddCommand}
              class="px-3 py-1 text-xs font-medium text-white bg-blue-600 rounded hover:bg-blue-700"
            >
              {$_('settings.editorTab.addCommand')}
            </button>
          </div>
        </div>

        <!-- Commands table -->
        <div class="space-y-2">
          {#each quickCommandsList as cmd (cmd.id)}
            <div class="flex items-center gap-3 p-2 bg-white dark:bg-gray-800 rounded border border-gray-200 dark:border-gray-600">
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2">
                  <code class="text-sm font-mono text-blue-600 dark:text-blue-400">{cmd.trigger}</code>
                  <span class="text-xs text-gray-500 dark:text-gray-400">→</span>
                  <code class="text-sm font-mono text-gray-600 dark:text-gray-300 truncate">{getPreview(cmd)}</code>
                </div>
                <p class="text-xs text-gray-500 dark:text-gray-400 truncate">{cmd.description}</p>
              </div>
              <div class="flex gap-1">
                <button
                  type="button"
                  on:click={() => startEditCommand(cmd)}
                  class="p-1.5 text-gray-500 hover:text-blue-600 dark:hover:text-blue-400"
                  title={$_('common.edit')}
                >
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                  </svg>
                </button>
                <button
                  type="button"
                  on:click={() => deleteCommand(cmd.id)}
                  class="p-1.5 text-gray-500 hover:text-red-600 dark:hover:text-red-400"
                  title={$_('common.delete')}
                >
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                  </svg>
                </button>
              </div>
            </div>
          {/each}

          {#if quickCommandsList.length === 0}
            <p class="text-sm text-gray-500 dark:text-gray-400 text-center py-4">
              {$_('settings.editorTab.noCommands')}
            </p>
          {/if}
        </div>
      </div>

      <!-- Edit form (inline) -->
      {#if editingCommand}
        <div class="bg-blue-50 dark:bg-blue-900/20 rounded-lg p-4 border border-blue-200 dark:border-blue-700">
          <h4 class="text-sm font-medium text-gray-900 dark:text-white mb-3">
            {isNewCommand ? $_('settings.editorTab.addNewCommand') : $_('settings.editorTab.editCommand')}
          </h4>

          <div class="grid grid-cols-1 tablet:grid-cols-2 gap-4 mb-4">
            <!-- Trigger -->
            <div>
              <label for="cmd-trigger" class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                {$_('settings.editorTab.trigger')}
              </label>
              <input
                id="cmd-trigger"
                type="text"
                bind:value={editingCommand.trigger}
                placeholder="/mycommand"
                class="w-full px-3 py-2 text-sm bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded focus:ring-2 focus:ring-blue-500 font-mono"
              />
            </div>

            <!-- Type -->
            <div>
              <label for="cmd-type" class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                {$_('settings.editorTab.type')}
              </label>
              <select
                id="cmd-type"
                bind:value={editingCommand.type}
                class="w-full px-3 py-2 text-sm bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded focus:ring-2 focus:ring-blue-500"
              >
                {#each commandTypes as type}
                  <option value={type.value}>{type.label}</option>
                {/each}
              </select>
            </div>

            <!-- Format (for datetime/date/time) -->
            {#if editingCommand.type === 'datetime' || editingCommand.type === 'date' || editingCommand.type === 'time'}
              <div>
                <label for="cmd-format" class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                  {$_('settings.editorTab.format')}
                </label>
                <input
                  id="cmd-format"
                  type="text"
                  bind:value={editingCommand.format}
                  placeholder="YYYY-MM-DD HH:mm:ss"
                  class="w-full px-3 py-2 text-sm bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded focus:ring-2 focus:ring-blue-500 font-mono"
                />
                <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
                  {$_('settings.editorTab.formatHelp')}
                </p>
              </div>
            {/if}

            <!-- Value (for text) -->
            {#if editingCommand.type === 'text'}
              <div>
                <label for="cmd-value" class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                  {$_('settings.editorTab.value')}
                </label>
                <input
                  id="cmd-value"
                  type="text"
                  bind:value={editingCommand.value}
                  placeholder="Text to insert"
                  class="w-full px-3 py-2 text-sm bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded focus:ring-2 focus:ring-blue-500"
                />
              </div>
            {/if}

            <!-- Description -->
            <div class="tablet:col-span-2">
              <label for="cmd-description" class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                {$_('settings.editorTab.description')}
              </label>
              <input
                id="cmd-description"
                type="text"
                bind:value={editingCommand.description}
                placeholder="Description shown in autocomplete"
                class="w-full px-3 py-2 text-sm bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded focus:ring-2 focus:ring-blue-500"
              />
            </div>
          </div>

          <!-- Preview -->
          <div class="mb-4 p-2 bg-gray-100 dark:bg-gray-800 rounded">
            <span class="text-xs text-gray-500 dark:text-gray-400">{$_('settings.editorTab.preview')}:</span>
            <code class="ml-2 text-sm font-mono text-gray-900 dark:text-white">{getPreview(editingCommand)}</code>
          </div>

          <!-- Buttons -->
          <div class="flex gap-2 justify-end">
            <button
              type="button"
              on:click={cancelEdit}
              class="px-3 py-1.5 text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-600 border border-gray-300 dark:border-gray-500 rounded hover:bg-gray-50 dark:hover:bg-gray-500"
            >
              {$_('common.cancel')}
            </button>
            <button
              type="button"
              on:click={saveCommand}
              class="px-3 py-1.5 text-sm font-medium text-white bg-blue-600 rounded hover:bg-blue-700"
            >
              {$_('common.save')}
            </button>
          </div>
        </div>
      {/if}
    {/if}
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
