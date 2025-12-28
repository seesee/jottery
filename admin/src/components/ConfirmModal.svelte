<script lang="ts">
  interface Props {
    title: string;
    message: string;
    confirmText?: string;
    cancelText?: string;
    onConfirm: () => void;
    onCancel: () => void;
  }

  let {
    title,
    message,
    confirmText = 'Confirm',
    cancelText = 'Cancel',
    onConfirm,
    onCancel,
  }: Props = $props();
</script>

<div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
  <div class="bg-white rounded-lg shadow-xl max-w-md w-full mx-4">
    <div class="p-6">
      <h3 class="text-lg font-semibold text-gray-900 mb-2">{title}</h3>
      <p class="text-gray-600">{message}</p>
    </div>

    <div class="bg-gray-50 px-6 py-4 flex justify-end space-x-3 rounded-b-lg">
      <button
        onclick={onCancel}
        class="px-4 py-2 bg-gray-200 text-gray-700 rounded-md hover:bg-gray-300 transition-colors"
      >
        {cancelText}
      </button>
      <button
        onclick={onConfirm}
        class="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 transition-colors"
      >
        {confirmText}
      </button>
    </div>
  </div>
</div>
