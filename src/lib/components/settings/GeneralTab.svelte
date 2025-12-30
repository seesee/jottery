<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { Theme } from '../../types';
  import { AVAILABLE_LOCALES } from '../../services';

  export let theme: Theme;
  export let layoutMode: 'auto' | 'mobile' | 'desktop';
  export let fontSize: 'auto' | 'small' | 'medium' | 'large';
  export let autoLockTimeout: number;
  export let sortOrder: 'recent' | 'created' | 'oldest' | 'alpha';
  export let language: string;
  export let rememberPassword: boolean;
  export let onRememberPasswordToggle: () => void;
</script>

<!-- Language -->
<div>
  <label for="setting-language" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
    {$_('settings.language')}
  </label>
  <select
    id="setting-language"
    bind:value={language}
    class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
  >
    {#each AVAILABLE_LOCALES as { code, name }}
      <option value={code}>{name}</option>
    {/each}
  </select>
</div>

<!-- Theme -->
<div>
  <label for="setting-theme" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
    {$_('settings.theme')}
  </label>
  <select
    id="setting-theme"
    bind:value={theme}
    class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
  >
    <option value="auto">{$_('settings.themeAuto')}</option>
    <option value="light">{$_('settings.themeLight')}</option>
    <option value="dark">{$_('settings.themeDark')}</option>
  </select>
</div>

<!-- Layout Mode -->
<div>
  <label for="setting-layout-mode" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
    Layout Mode
  </label>
  <select
    id="setting-layout-mode"
    bind:value={layoutMode}
    class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
  >
    <option value="auto">Auto (Responsive)</option>
    <option value="mobile">Force Mobile Layout</option>
    <option value="desktop">Force Desktop Layout</option>
  </select>
  <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
    Override the automatic layout detection for this device
  </p>
</div>

<!-- Font Size -->
<div>
  <label for="setting-font-size" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
    Editor Font Size
  </label>
  <select
    id="setting-font-size"
    bind:value={fontSize}
    class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
  >
    <option value="auto">Auto (Mobile-aware: 16px on mobile, 14px desktop)</option>
    <option value="small">Small (12px)</option>
    <option value="medium">Medium (14px)</option>
    <option value="large">Large (16px)</option>
  </select>
  <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
    Auto uses larger font on mobile to prevent browser zoom
  </p>
</div>

<!-- Auto-lock timeout -->
<div>
  <label for="setting-autolock" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
    {$_('settings.autoLockTimeout')}
  </label>
  <input
    id="setting-autolock"
    type="number"
    bind:value={autoLockTimeout}
    min="1"
    max="1440"
    class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
  />
</div>

<!-- Sort order -->
<div>
  <label for="setting-sort-order" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
    {$_('settings.sortOrder')}
  </label>
  <select
    id="setting-sort-order"
    bind:value={sortOrder}
    class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
  >
    <option value="recent">{$_('settings.sortRecent')}</option>
    <option value="created">{$_('settings.sortCreated')}</option>
    <option value="oldest">{$_('settings.sortOldest')}</option>
    <option value="alpha">{$_('settings.sortAlpha')}</option>
  </select>
</div>

<!-- Remember Password Toggle -->
<div class="bg-orange-50 dark:bg-orange-900/20 border border-orange-200 dark:border-orange-800 rounded-lg p-4">
  <div class="flex items-start justify-between">
    <div class="flex-1">
      <h4 class="text-sm font-medium text-orange-800 dark:text-orange-200 mb-2">
        🔓 Remember Password (Insecure)
      </h4>
      <p class="text-sm text-orange-700 dark:text-orange-300 mb-2">
        Store your password on this device to skip entering it on every visit. <strong>WARNING:</strong> This stores your password in plain text in localStorage, which is highly insecure.
      </p>
      <p class="text-xs text-orange-600 dark:text-orange-400">
        Auto-lock will be disabled when this is enabled. Disabling this will immediately lock the application.
      </p>
    </div>
    <label class="relative inline-flex items-center cursor-pointer ml-4">
      <input
        type="checkbox"
        bind:checked={rememberPassword}
        on:change={onRememberPasswordToggle}
        class="sr-only peer"
      />
      <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-orange-300 dark:peer-focus:ring-orange-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-orange-600"></div>
    </label>
  </div>
</div>
