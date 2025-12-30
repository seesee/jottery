<script lang="ts">
  export let showDocumentation: boolean;
  export let onShowDocumentation: () => void;

  // Access the version from the global __APP_VERSION__ variable
  declare const __APP_VERSION__: string;
</script>

<div class="space-y-6">
  <!-- Version -->
  <div>
    <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Version</h4>
    <p class="text-lg text-gray-900 dark:text-white font-mono">v{__APP_VERSION__}</p>
  </div>

  <!-- Documentation -->
  <div>
    <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">Documentation</h4>
    <button
      on:click={onShowDocumentation}
      class="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-md transition-colors flex items-center justify-center gap-2"
    >
      📚 View Documentation
    </button>
    <p class="mt-2 text-sm text-gray-500 dark:text-gray-400">
      Learn how to use Jottery effectively
    </p>
  </div>

  <!-- Terminal Client Info -->
  <div>
    <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Terminal Client</h4>
    <p class="text-sm text-gray-600 dark:text-gray-400 mb-2">
      Access your notes from the command line with the Jottery terminal client. The TUI provides a fast, keyboard-driven interface for managing your notes.
    </p>
    <p class="text-sm text-gray-600 dark:text-gray-400">
      Download the terminal client from the <strong>Advanced</strong> tab.
    </p>
  </div>

  <!-- About -->
  <div>
    <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">About Jottery</h4>
    <p class="text-sm text-gray-600 dark:text-gray-400 mb-2">
      Jottery is a privacy-focused, self-hosted scratch pad application for capturing, organizing, and searching notes with rich content, syntax highlighting, and encryption.
    </p>
    <p class="text-sm text-gray-600 dark:text-gray-400">
      Licensed under the MIT License.
    </p>
  </div>
</div>
