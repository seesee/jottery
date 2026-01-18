<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { Theme } from '../../types';
  import { AVAILABLE_LOCALES, sessionStorageService } from '../../services';

  export let theme: Theme;
  export let layoutMode: 'auto' | 'mobile' | 'desktop';
  export let fontSize: 'auto' | 'small' | 'medium' | 'large';
  export let autoLockTimeout: number;
  export let sortOrder: 'recent' | 'created' | 'oldest' | 'alpha';
  export let language: string;
  export let timezone: string;
  export let rememberPassword: boolean;
  export let openLinksInNewTab: boolean;
  export let onRememberPasswordToggle: () => void;
  export let persistSession: boolean;
  export let persistSessionTimeout: number;
  export let onPersistSessionToggle: () => void;

  // Check if sessionStorage is available
  $: sessionStorageAvailable = sessionStorageService.isAvailable();

  // Common timezones grouped by region
  const TIMEZONES = [
    { value: 'local', label: 'Local (Browser Timezone)' },
    { value: 'UTC', label: 'UTC (Coordinated Universal Time)' },
    { value: 'America/New_York', label: 'America/New York (EST/EDT)' },
    { value: 'America/Chicago', label: 'America/Chicago (CST/CDT)' },
    { value: 'America/Denver', label: 'America/Denver (MST/MDT)' },
    { value: 'America/Los_Angeles', label: 'America/Los Angeles (PST/PDT)' },
    { value: 'America/Anchorage', label: 'America/Anchorage (AKST/AKDT)' },
    { value: 'Pacific/Honolulu', label: 'Pacific/Honolulu (HST)' },
    { value: 'America/Toronto', label: 'America/Toronto (EST/EDT)' },
    { value: 'America/Mexico_City', label: 'America/Mexico City (CST/CDT)' },
    { value: 'America/Sao_Paulo', label: 'America/São Paulo (BRT)' },
    { value: 'Europe/London', label: 'Europe/London (GMT/BST)' },
    { value: 'Europe/Paris', label: 'Europe/Paris (CET/CEST)' },
    { value: 'Europe/Berlin', label: 'Europe/Berlin (CET/CEST)' },
    { value: 'Europe/Rome', label: 'Europe/Rome (CET/CEST)' },
    { value: 'Europe/Madrid', label: 'Europe/Madrid (CET/CEST)' },
    { value: 'Europe/Amsterdam', label: 'Europe/Amsterdam (CET/CEST)' },
    { value: 'Europe/Stockholm', label: 'Europe/Stockholm (CET/CEST)' },
    { value: 'Europe/Moscow', label: 'Europe/Moscow (MSK)' },
    { value: 'Europe/Istanbul', label: 'Europe/Istanbul (TRT)' },
    { value: 'Asia/Dubai', label: 'Asia/Dubai (GST)' },
    { value: 'Asia/Kolkata', label: 'Asia/Kolkata (IST)' },
    { value: 'Asia/Bangkok', label: 'Asia/Bangkok (ICT)' },
    { value: 'Asia/Singapore', label: 'Asia/Singapore (SGT)' },
    { value: 'Asia/Hong_Kong', label: 'Asia/Hong Kong (HKT)' },
    { value: 'Asia/Shanghai', label: 'Asia/Shanghai (CST)' },
    { value: 'Asia/Tokyo', label: 'Asia/Tokyo (JST)' },
    { value: 'Asia/Seoul', label: 'Asia/Seoul (KST)' },
    { value: 'Australia/Sydney', label: 'Australia/Sydney (AEDT/AEST)' },
    { value: 'Australia/Melbourne', label: 'Australia/Melbourne (AEDT/AEST)' },
    { value: 'Australia/Brisbane', label: 'Australia/Brisbane (AEST)' },
    { value: 'Australia/Perth', label: 'Australia/Perth (AWST)' },
    { value: 'Pacific/Auckland', label: 'Pacific/Auckland (NZDT/NZST)' },
  ];
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

<!-- Timezone -->
<div>
  <label for="setting-timezone" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
    {$_('settings.timezone')}
  </label>
  <select
    id="setting-timezone"
    bind:value={timezone}
    class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
  >
    {#each TIMEZONES as tz}
      <option value={tz.value}>{tz.label}</option>
    {/each}
  </select>
  <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
    {$_('settings.timezoneDescription')}
  </p>
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
    {$_('settings.layoutMode')}
  </label>
  <select
    id="setting-layout-mode"
    bind:value={layoutMode}
    class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
  >
    <option value="auto">{$_('settings.layoutModeAuto')}</option>
    <option value="mobile">{$_('settings.layoutModeMobile')}</option>
    <option value="desktop">{$_('settings.layoutModeDesktop')}</option>
  </select>
  <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
    {$_('settings.layoutModeDescription')}
  </p>
</div>

<!-- Font Size -->
<div>
  <label for="setting-font-size" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
    {$_('settings.fontSize')}
  </label>
  <select
    id="setting-font-size"
    bind:value={fontSize}
    class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
  >
    <option value="auto">{$_('settings.fontSizeAuto')}</option>
    <option value="small">{$_('settings.fontSizeSmall')}</option>
    <option value="medium">{$_('settings.fontSizeMedium')}</option>
    <option value="large">{$_('settings.fontSizeLarge')}</option>
  </select>
  <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
    {$_('settings.fontSizeDescription')}
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

<!-- Open Links in New Tab Toggle -->
<div class="flex items-center justify-between">
  <div class="flex-1">
    <label for="setting-open-links-new-tab" class="block text-sm font-medium text-gray-700 dark:text-gray-300">
      {$_('settings.openLinksInNewTab')}
    </label>
    <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
      {$_('settings.openLinksInNewTabDescription')}
    </p>
  </div>
  <label class="relative inline-flex items-center cursor-pointer ml-4">
    <input
      id="setting-open-links-new-tab"
      type="checkbox"
      bind:checked={openLinksInNewTab}
      class="sr-only peer"
    />
    <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
  </label>
</div>

<!-- Persist Session Toggle (Tab-scoped, auto-expires) -->
<div class="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4">
  <div class="flex items-start justify-between">
    <div class="flex-1">
      <h4 class="text-sm font-medium text-blue-800 dark:text-blue-200 mb-2">
        🔄 {$_('settings.persistSession')}
      </h4>
      <p class="text-sm text-blue-700 dark:text-blue-300 mb-2">
        {$_('settings.persistSessionDescription')}
      </p>
      <p class="text-xs text-blue-600 dark:text-blue-400">
        {$_('settings.persistSessionNote')}
      </p>
      {#if !sessionStorageAvailable}
        <p class="text-xs text-red-600 dark:text-red-400 mt-2">
          ⚠️ {$_('settings.persistSessionUnavailable')}
        </p>
      {:else if rememberPassword}
        <p class="text-xs text-gray-500 dark:text-gray-400 mt-2">
          {$_('settings.persistSessionDisabledByRemember')}
        </p>
      {/if}
    </div>
    <label class="relative inline-flex items-center cursor-pointer ml-4">
      <input
        type="checkbox"
        bind:checked={persistSession}
        on:change={onPersistSessionToggle}
        disabled={!sessionStorageAvailable || rememberPassword}
        class="sr-only peer"
      />
      <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600 peer-disabled:opacity-50 peer-disabled:cursor-not-allowed"></div>
    </label>
  </div>

  <!-- Session timeout input (only visible when persistSession is enabled) -->
  {#if persistSession && sessionStorageAvailable && !rememberPassword}
    <div class="mt-4 pt-4 border-t border-blue-200 dark:border-blue-700">
      <label for="setting-persist-session-timeout" class="block text-sm font-medium text-blue-800 dark:text-blue-200 mb-2">
        {$_('settings.persistSessionTimeout')}
      </label>
      <input
        id="setting-persist-session-timeout"
        type="number"
        bind:value={persistSessionTimeout}
        min="5"
        max="480"
        class="w-full px-3 py-2 border border-blue-300 dark:border-blue-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
      />
      <p class="mt-1 text-xs text-blue-600 dark:text-blue-400">
        {$_('settings.persistSessionTimeoutDescription')}
      </p>
    </div>
  {/if}
</div>

<!-- Remember Password Toggle -->
<div class="bg-orange-50 dark:bg-orange-900/20 border border-orange-200 dark:border-orange-800 rounded-lg p-4">
  <div class="flex items-start justify-between">
    <div class="flex-1">
      <h4 class="text-sm font-medium text-orange-800 dark:text-orange-200 mb-2">
        🔓 {$_('settings.rememberPassword')}
      </h4>
      <p class="text-sm text-orange-700 dark:text-orange-300 mb-2">
        {@html $_('settings.rememberPasswordWarning')}
      </p>
      <p class="text-xs text-orange-600 dark:text-orange-400">
        {$_('settings.rememberPasswordNote')}
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
