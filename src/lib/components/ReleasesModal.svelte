<script lang="ts">
  import { _ } from 'svelte-i18n';

  export let show = false;
  export let onClose: () => void = () => {};

  const githubRepo = 'https://github.com/seesee/jottery';

  // TUI client downloads
  const tuiReleases = [
    { name: 'Linux (x64)', url: `${githubRepo}/releases/latest/download/jottery-linux-x64` },
    { name: 'Linux (arm64)', url: `${githubRepo}/releases/latest/download/jottery-linux-arm64` },
    { name: 'Linux (armv7)', url: `${githubRepo}/releases/latest/download/jottery-linux-armv7` },
    { name: 'macOS', url: `${githubRepo}/releases/latest/download/jottery-macos` },
    { name: 'Windows', url: `${githubRepo}/releases/latest/download/jottery-windows.exe` },
    { name: 'All platforms (zip)', url: `${githubRepo}/releases/latest/download/jottery-tui.zip` },
  ];

  // Standalone web app download
  const webAppRelease = {
    name: 'Standalone HTML (zip)',
    url: `${githubRepo}/releases/latest/download/jottery-web.zip`,
    note: 'releases.webAppNote'
  };
</script>

{#if show}
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="fixed inset-0 bg-black bg-opacity-50 z-50 flex items-center justify-center p-4" on:click={onClose}>
    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl p-6 w-full max-w-md" on:click|stopPropagation>
      <h2 class="text-xl font-bold mb-4 text-gray-900 dark:text-white">{$_('releases.title')}</h2>
      <p class="mb-4 text-gray-700 dark:text-gray-300">{$_('releases.description')}</p>

      <!-- TUI Client Downloads -->
      <div class="mb-4">
        <h3 class="text-sm font-semibold mb-2 text-gray-900 dark:text-white">{$_('releases.tuiClient')}</h3>
        <ul class="space-y-1.5">
          {#each tuiReleases as release}
            <li>
              <a
                href={release.url}
                class="text-blue-600 dark:text-blue-400 hover:underline flex items-center gap-2 text-sm"
                target="_blank"
                rel="noopener noreferrer"
              >
                <span>↓</span>
                {release.name}
              </a>
            </li>
          {/each}
        </ul>
      </div>

      <!-- Standalone Web App Download -->
      <div class="mb-6">
        <h3 class="text-sm font-semibold mb-2 text-gray-900 dark:text-white">{$_('releases.standaloneWebApp')}</h3>
        <a
          href={webAppRelease.url}
          class="text-blue-600 dark:text-blue-400 hover:underline flex items-center gap-2 text-sm"
          target="_blank"
          rel="noopener noreferrer"
        >
          <span>↓</span>
          {webAppRelease.name}
        </a>
        <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
          {$_('releases.webAppNote')}
        </p>
      </div>

      <div class="border-t border-gray-200 dark:border-gray-700 pt-4 mb-4">
        <p class="text-sm text-gray-600 dark:text-gray-400 mb-2">
          <a
            href={githubRepo}
            target="_blank"
            rel="noopener noreferrer"
            class="text-blue-600 dark:text-blue-400 hover:underline"
          >
            View on GitHub
          </a>
        </p>
        <p class="text-xs text-gray-500 dark:text-gray-500">
          © 2025 jottery • MIT License
        </p>
      </div>

      <div class="text-right">
        <button
          on:click={onClose}
          class="px-4 py-2 bg-gray-200 dark:bg-gray-600 text-gray-900 dark:text-white rounded-md hover:bg-gray-300 dark:hover:bg-gray-500 transition-colors"
        >
          {$_('common.close')}
        </button>
      </div>
    </div>
  </div>
{/if}
