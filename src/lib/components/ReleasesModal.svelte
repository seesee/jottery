<script lang="ts">
  import { _ } from 'svelte-i18n';

  export let show = false;
  export let onClose: () => void = () => {};

  const releases = [
    { name: 'Linux (amd64)', path: '/releases/jottery-linux-amd64' },
    { name: 'macOS (amd64)', path: '/releases/jottery-macos-amd64' },
    { name: 'Windows (amd64)', path: '/releases/jottery-windows-amd64.exe' },
  ];
</script>

{#if show}
  <div class="fixed inset-0 bg-black bg-opacity-50 z-50 flex items-center justify-center" on:click={onClose}>
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl p-6 w-full max-w-md" on:click|stopPropagation>
      <h2 class="text-xl font-bold mb-4">{$_('releases.title')}</h2>
      <p class="mb-4">{$_('releases.description')}</p>
      <ul>
        {#each releases as release}
          <li class="mb-2">
            <a href={release.path} download class="text-blue-600 hover:underline">
              {release.name}
            </a>
          </li>
        {/each}
      </ul>
      <div class="mt-6 text-right">
        <button on:click={onClose} class="px-4 py-2 bg-gray-200 dark:bg-gray-600 rounded-md">
          {$_('common.close')}
        </button>
      </div>
    </div>
  </div>
{/if}
