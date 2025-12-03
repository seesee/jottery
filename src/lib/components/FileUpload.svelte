<script lang="ts">
  import { attachmentService } from '../services';

  export let onUpload: (files: FileList) => void;
  export let disabled: boolean = false;

  let isDragging = false;
  let fileInput: HTMLInputElement;

  function handleDragEnter(e: DragEvent) {
    if (disabled) return;
    e.preventDefault();
    e.stopPropagation();
    isDragging = true;
  }

  function handleDragLeave(e: DragEvent) {
    if (disabled) return;
    e.preventDefault();
    e.stopPropagation();

    // Only set isDragging to false if we're leaving the drop zone
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const x = e.clientX;
    const y = e.clientY;

    if (x < rect.left || x >= rect.right || y < rect.top || y >= rect.bottom) {
      isDragging = false;
    }
  }

  function handleDragOver(e: DragEvent) {
    if (disabled) return;
    e.preventDefault();
    e.stopPropagation();
  }

  function handleDrop(e: DragEvent) {
    if (disabled) return;
    e.preventDefault();
    e.stopPropagation();
    isDragging = false;

    const files = e.dataTransfer?.files;
    if (files && files.length > 0) {
      onUpload(files);
    }
  }

  function handleFileSelect(e: Event) {
    if (disabled) return;
    const target = e.target as HTMLInputElement;
    const files = target.files;
    if (files && files.length > 0) {
      onUpload(files);
      // Clear the input so the same file can be selected again
      target.value = '';
    }
  }

  function openFileDialog() {
    if (disabled) return;
    fileInput?.click();
  }
</script>

<div
  class="border-2 border-dashed rounded-lg p-4 text-center transition-colors {isDragging
    ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/20'
    : 'border-gray-300 dark:border-gray-600 bg-gray-50 dark:bg-gray-800'} {disabled
    ? 'opacity-50 cursor-not-allowed'
    : 'cursor-pointer hover:border-blue-400 hover:bg-gray-100 dark:hover:bg-gray-700'}"
  on:dragenter={handleDragEnter}
  on:dragleave={handleDragLeave}
  on:dragover={handleDragOver}
  on:drop={handleDrop}
  on:click={openFileDialog}
  role="button"
  tabindex="0"
  on:keydown={(e) => e.key === 'Enter' && openFileDialog()}
>
  <input
    type="file"
    bind:this={fileInput}
    on:change={handleFileSelect}
    multiple
    class="hidden"
    {disabled}
  />

  <div class="text-gray-600 dark:text-gray-400">
    <div class="text-2xl mb-2">📎</div>
    <div class="text-sm">
      {#if isDragging}
        <span class="font-medium text-blue-600 dark:text-blue-400">Drop files here</span>
      {:else}
        <span class="font-medium">Click to upload</span> or drag and drop
      {/if}
    </div>
    <div class="text-xs text-gray-500 dark:text-gray-500 mt-1">
      Max file size: 10MB
    </div>
  </div>
</div>
