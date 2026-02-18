<script lang="ts">
  /**
   * ResizableSplitter — a thin vertical drag handle between the sidebar and editor pane.
   * Supports mouse drag to resize. Persists the new width via a callback on mouseup.
   */

  export let width: number;
  export let minWidth = 200;
  export let maxWidth = 500;
  export let onWidthChange: (newWidth: number) => void;
  export let onWidthCommit: (newWidth: number) => void;

  let dragging = false;
  let startX = 0;
  let startWidth = 0;

  function handleMouseDown(event: MouseEvent) {
    event.preventDefault();
    dragging = true;
    startX = event.clientX;
    startWidth = width;

    // Attach listeners to window so dragging works even when the cursor leaves the splitter
    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);
  }

  function handleMouseMove(event: MouseEvent) {
    if (!dragging) return;
    const delta = event.clientX - startX;
    const newWidth = Math.min(maxWidth, Math.max(minWidth, startWidth + delta));
    onWidthChange(newWidth);
  }

  function handleMouseUp() {
    if (!dragging) return;
    dragging = false;
    window.removeEventListener('mousemove', handleMouseMove);
    window.removeEventListener('mouseup', handleMouseUp);
    // Persist the final width to settings
    onWidthCommit(width);
  }
</script>

<!-- svelte-ignore a11y-no-static-element-interactions a11y-no-noninteractive-element-interactions -->
<div
  class="splitter"
  class:dragging
  on:mousedown={handleMouseDown}
  role="separator"
  aria-orientation="vertical"
  aria-valuenow={width}
  aria-valuemin={minWidth}
  aria-valuemax={maxWidth}
></div>

<style>
  .splitter {
    width: 4px;
    flex-shrink: 0;
    cursor: col-resize;
    background-color: transparent;
    transition: background-color 150ms ease;
    /* Widen the hit area without changing visual width */
    position: relative;
    z-index: 10;
  }

  .splitter::before {
    content: '';
    position: absolute;
    top: 0;
    bottom: 0;
    left: -3px;
    right: -3px;
  }

  .splitter:hover {
    background-color: rgba(156, 163, 175, 0.4); /* gray-400 at 40% opacity */
  }

  :global(.dark) .splitter:hover {
    background-color: rgba(107, 114, 128, 0.5); /* gray-500 at 50% opacity */
  }

  .splitter.dragging {
    background-color: rgba(59, 130, 246, 0.6); /* blue-500 at 60% opacity */
  }

  :global(.dark) .splitter.dragging {
    background-color: rgba(96, 165, 250, 0.6); /* blue-400 at 60% opacity */
  }
</style>
