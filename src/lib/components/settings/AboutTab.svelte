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
    <p class="text-sm text-gray-600 dark:text-gray-400 mb-3">
      Jottery is a privacy-focused, self-hosted scratch pad application for capturing, organizing, and searching notes with rich content, syntax highlighting, and encryption.
    </p>
    <a
      href="https://github.com/seesee/jottery"
      target="_blank"
      rel="noopener noreferrer"
      class="inline-flex items-center text-blue-600 dark:text-blue-400 hover:text-blue-800 dark:hover:text-blue-300 font-medium text-sm mb-3"
    >
      <svg class="w-4 h-4 mr-1.5" fill="currentColor" viewBox="0 0 24 24">
        <path fill-rule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clip-rule="evenodd"/>
      </svg>
      View on GitHub
    </a>
    <p class="text-sm text-gray-600 dark:text-gray-400">
      Licensed under the MIT License.
    </p>
  </div>
</div>
