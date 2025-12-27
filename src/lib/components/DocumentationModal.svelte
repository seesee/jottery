<script lang="ts">
  export let show = false;
  export let onClose: () => void = () => {};

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      onClose();
    }
  }
</script>

{#if show}
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-0 tablet:p-4"
    on:click={handleBackdropClick}
    on:keydown={(e) => e.key === 'Escape' && onClose()}
    role="dialog"
    aria-modal="true"
    tabindex="0"
  >
    <div class="bg-white dark:bg-gray-800 w-full h-full tablet:h-auto tablet:max-w-3xl tablet:rounded-lg shadow-xl tablet:max-h-[90vh] flex flex-col">
      <!-- Header -->
      <div class="border-b border-gray-200 dark:border-gray-700 p-4 flex items-center justify-between flex-shrink-0">
        <h2 class="text-xl font-bold text-gray-900 dark:text-white">📚 Documentation</h2>
        <button
          on:click={onClose}
          class="min-h-11 min-w-11 p-3 -m-2 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors text-gray-500 dark:text-gray-400"
          aria-label="Close documentation"
        >
          ✕
        </button>
      </div>

      <!-- Content (scrollable) -->
      <div class="flex-1 overflow-y-auto p-6 space-y-6">
        <section>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-3">Getting Started</h3>
          <p class="text-gray-700 dark:text-gray-300 mb-3">
            Jottery is a privacy-focused, encrypted note-taking application. All your notes are encrypted locally using AES-256-GCM encryption before being stored.
          </p>
          <p class="text-gray-700 dark:text-gray-300">
            <strong>Important:</strong> Your password is the encryption key. If you lose it, your notes cannot be recovered.
          </p>
        </section>

        <section>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-3">Creating & Editing Notes</h3>
          <ul class="list-disc list-inside space-y-2 text-gray-700 dark:text-gray-300">
            <li><strong>Create a note:</strong> Click the "+ New Note" button or press <kbd class="px-2 py-1 bg-gray-200 dark:bg-gray-700 rounded text-xs">Alt+N</kbd></li>
            <li><strong>Edit a note:</strong> Click on a note in the list to open it in the editor</li>
            <li><strong>Auto-save:</strong> Changes are automatically saved as you type</li>
            <li><strong>Close a note:</strong> Press <kbd class="px-2 py-1 bg-gray-200 dark:bg-gray-700 rounded text-xs">Escape</kbd> or click another note</li>
          </ul>
        </section>

        <section>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-3">Syntax Highlighting</h3>
          <p class="text-gray-700 dark:text-gray-300 mb-3">
            Use the language dropdown in the editor to enable syntax highlighting for:
          </p>
          <ul class="list-disc list-inside space-y-1 text-gray-700 dark:text-gray-300">
            <li>Markdown (with code block syntax highlighting)</li>
            <li>JavaScript/TypeScript</li>
            <li>Python</li>
            <li>JSON, HTML, CSS, SQL</li>
            <li>Bash/Shell, Perl</li>
          </ul>
        </section>

        <section>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-3">Search</h3>
          <p class="text-gray-700 dark:text-gray-300 mb-3">
            Jottery supports powerful search syntax:
          </p>
          <ul class="list-disc list-inside space-y-2 text-gray-700 dark:text-gray-300">
            <li><code class="bg-gray-100 dark:bg-gray-900 px-2 py-1 rounded">#tagname</code> - Find notes with a specific tag</li>
            <li><code class="bg-gray-100 dark:bg-gray-900 px-2 py-1 rounded">#tag1 #tag2</code> - Notes with both tags (AND)</li>
            <li><code class="bg-gray-100 dark:bg-gray-900 px-2 py-1 rounded">#tag1 | #tag2</code> - Notes with either tag (OR)</li>
            <li><code class="bg-gray-100 dark:bg-gray-900 px-2 py-1 rounded">"exact phrase"</code> - Search for exact phrase</li>
            <li><code class="bg-gray-100 dark:bg-gray-900 px-2 py-1 rounded">-word</code> - Exclude notes containing "word"</li>
          </ul>
        </section>

        <section>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-3">Version History</h3>
          <p class="text-gray-700 dark:text-gray-300 mb-3">
            Jottery automatically creates version snapshots when you close or switch between notes. Access version history:
          </p>
          <ul class="list-disc list-inside space-y-2 text-gray-700 dark:text-gray-300">
            <li>Click the ⋮ menu in the editor and select "📜 Version History"</li>
            <li>Or press <kbd class="px-2 py-1 bg-gray-200 dark:bg-gray-700 rounded text-xs">Alt+H</kbd></li>
            <li>View previous versions, compare changes, and restore old content</li>
          </ul>
        </section>

        <section>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-3">Keyboard Shortcuts</h3>
          <p class="text-gray-700 dark:text-gray-300 mb-3">
            All keyboard shortcuts are customizable in Settings. Default shortcuts:
          </p>
          <table class="w-full text-sm text-gray-700 dark:text-gray-300">
            <tbody class="divide-y divide-gray-200 dark:divide-gray-700">
              <tr><td class="py-2 pr-4"><kbd class="px-2 py-1 bg-gray-200 dark:bg-gray-700 rounded text-xs">Ctrl/Cmd+K</kbd></td><td>Focus search</td></tr>
              <tr><td class="py-2 pr-4"><kbd class="px-2 py-1 bg-gray-200 dark:bg-gray-700 rounded text-xs">Alt+N</kbd></td><td>Create new note</td></tr>
              <tr><td class="py-2 pr-4"><kbd class="px-2 py-1 bg-gray-200 dark:bg-gray-700 rounded text-xs">Ctrl/Cmd+Z</kbd></td><td>Undo</td></tr>
              <tr><td class="py-2 pr-4"><kbd class="px-2 py-1 bg-gray-200 dark:bg-gray-700 rounded text-xs">Ctrl/Cmd+Shift+Z</kbd></td><td>Redo</td></tr>
              <tr><td class="py-2 pr-4"><kbd class="px-2 py-1 bg-gray-200 dark:bg-gray-700 rounded text-xs">Alt+H</kbd></td><td>Version history</td></tr>
              <tr><td class="py-2 pr-4"><kbd class="px-2 py-1 bg-gray-200 dark:bg-gray-700 rounded text-xs">Alt+I</kbd></td><td>Note info</td></tr>
              <tr><td class="py-2 pr-4"><kbd class="px-2 py-1 bg-gray-200 dark:bg-gray-700 rounded text-xs">Escape</kbd></td><td>Close note</td></tr>
            </tbody>
          </table>
        </section>

        <section>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-3">Sync</h3>
          <p class="text-gray-700 dark:text-gray-300 mb-3">
            Jottery supports self-hosted sync across devices:
          </p>
          <ul class="list-disc list-inside space-y-2 text-gray-700 dark:text-gray-300">
            <li><strong>Setup:</strong> Go to Settings → Sync and enter your self-hosted server URL</li>
            <li><strong>Register:</strong> Register the first device to create sync credentials</li>
            <li><strong>Other devices:</strong> Use "Use Existing Credentials" to connect additional devices</li>
            <li><strong>Important:</strong> All devices must use the <strong>same password</strong> to decrypt notes</li>
          </ul>
        </section>

        <section>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-3">Security & Privacy</h3>
          <ul class="list-disc list-inside space-y-2 text-gray-700 dark:text-gray-300">
            <li>All note content and tags are encrypted with AES-256-GCM</li>
            <li>Encryption happens locally in your browser before storage</li>
            <li>Your password is never stored or transmitted</li>
            <li>Auto-lock protects your notes when you're away (default: 15 minutes)</li>
            <li>No analytics, tracking, or third-party scripts</li>
          </ul>
        </section>

        <section>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-3">Import & Export</h3>
          <p class="text-gray-700 dark:text-gray-300 mb-3">
            Export your notes as decrypted JSON or import from previous exports:
          </p>
          <ul class="list-disc list-inside space-y-2 text-gray-700 dark:text-gray-300">
            <li>Go to Settings → Import/Export</li>
            <li>Exports are unencrypted - store them securely</li>
            <li>Import will merge notes with existing data (duplicates are skipped)</li>
          </ul>
        </section>
      </div>

      <!-- Footer -->
      <div class="border-t border-gray-200 dark:border-gray-700 p-4 flex justify-end flex-shrink-0">
        <button
          on:click={onClose}
          class="px-4 py-2.5 min-h-11 bg-blue-600 active:bg-blue-700 text-white font-medium rounded-md transition-colors"
        >
          Close
        </button>
      </div>
    </div>
  </div>
{/if}
