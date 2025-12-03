<script lang="ts">
  export let show: boolean = false;
  export let title: string = 'Confirm';
  export let message: string;
  export let confirmText: string = 'Confirm';
  export let cancelText: string = 'Cancel';
  export let confirmClass: string = 'bg-red-600 hover:bg-red-700';
  export let requireTextMatch: string | null = null; // e.g., "DELETE"
  export let onConfirm: () => void;
  export let onCancel: () => void;

  let inputValue = '';

  $: canConfirm = requireTextMatch ? inputValue === requireTextMatch : true;

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      handleCancel();
    }
  }

  function handleConfirm() {
    if (canConfirm) {
      inputValue = '';
      onConfirm();
    }
  }

  function handleCancel() {
    inputValue = '';
    onCancel();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      handleCancel();
    } else if (e.key === 'Enter' && canConfirm) {
      handleConfirm();
    }
  }
</script>

{#if show}
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
    on:click={handleBackdropClick}
    on:keydown={handleKeydown}
    role="dialog"
    aria-modal="true"
  >
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full mx-4">
      <!-- Header -->
      <div class="border-b border-gray-200 dark:border-gray-700 p-4">
        <h2 class="text-xl font-bold text-gray-900 dark:text-white">{title}</h2>
      </div>

      <!-- Content -->
      <div class="p-6">
        <p class="text-gray-700 dark:text-gray-300 whitespace-pre-line">
          {message}
        </p>

        {#if requireTextMatch}
          <div class="mt-4">
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              Type <strong>{requireTextMatch}</strong> to confirm:
            </label>
            <input
              type="text"
              bind:value={inputValue}
              placeholder={requireTextMatch}
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-red-500"
              autofocus
            />
          </div>
        {/if}
      </div>

      <!-- Footer -->
      <div class="border-t border-gray-200 dark:border-gray-700 p-4 flex justify-end gap-2">
        <button
          on:click={handleCancel}
          class="px-4 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-md transition-colors"
        >
          {cancelText}
        </button>
        <button
          on:click={handleConfirm}
          disabled={!canConfirm}
          class="{confirmClass} px-4 py-2 text-white font-medium rounded-md transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {confirmText}
        </button>
      </div>
    </div>
  </div>
{/if}
