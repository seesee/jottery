<script lang="ts">
  import { _ } from 'svelte-i18n';

  export let onRefresh: () => Promise<void>;
  export let enabled: boolean = true;

  let container: HTMLElement;
  let isPulling = false;
  let pullDistance = 0;
  let isRefreshing = false;

  const PULL_THRESHOLD = 80; // pixels
  const MAX_PULL = 120; // pixels

  let touchStartY = 0;
  let touchStartScrollTop = 0;

  function handleTouchStart(e: TouchEvent) {
    if (!enabled || isRefreshing) return;

    // Only trigger if at top of scroll
    if (container.scrollTop === 0) {
      touchStartY = e.touches[0].clientY;
      touchStartScrollTop = container.scrollTop;
    }
  }

  function handleTouchMove(e: TouchEvent) {
    if (!enabled || touchStartY === 0 || isRefreshing) return;

    const currentY = e.touches[0].clientY;
    const distance = currentY - touchStartY;

    // Only pull down when at the top of scroll
    if (distance > 0 && container.scrollTop === 0) {
      // Prevent default scroll
      e.preventDefault();

      isPulling = true;
      pullDistance = Math.min(distance * 0.5, MAX_PULL); // Apply resistance
    }
  }

  async function handleTouchEnd() {
    if (!isPulling || !enabled) {
      touchStartY = 0;
      touchStartScrollTop = 0;
      isPulling = false;
      pullDistance = 0;
      return;
    }

    if (pullDistance >= PULL_THRESHOLD) {
      // Trigger refresh
      isRefreshing = true;
      pullDistance = PULL_THRESHOLD;

      try {
        await onRefresh();
      } finally {
        isRefreshing = false;
        isPulling = false;
        pullDistance = 0;
        touchStartY = 0;
        touchStartScrollTop = 0;
      }
    } else {
      // Snap back
      isPulling = false;
      pullDistance = 0;
      touchStartY = 0;
      touchStartScrollTop = 0;
    }
  }
</script>

<div
  bind:this={container}
  class="pull-to-refresh-container"
  on:touchstart={handleTouchStart}
  on:touchmove={handleTouchMove}
  on:touchend={handleTouchEnd}
  role="region"
  aria-label={$_('pullToRefresh.region')}
>
  {#if isPulling || isRefreshing}
    <div
      class="pull-indicator"
      style="transform: translateY({pullDistance}px); opacity: {Math.min(pullDistance / PULL_THRESHOLD, 1)}"
    >
      {#if isRefreshing}
        <div class="spinner"></div>
        <span>{$_('pullToRefresh.syncing')}</span>
      {:else if pullDistance >= PULL_THRESHOLD}
        <span>{$_('pullToRefresh.release')}</span>
      {:else}
        <span>{$_('pullToRefresh.pull')}</span>
      {/if}
    </div>
  {/if}

  <div
    class="content"
    style="transform: translateY({pullDistance}px); transition: {isPulling ? 'none' : 'transform 0.3s ease-out'}"
  >
    <slot />
  </div>
</div>

<style>
  .pull-to-refresh-container {
    height: 100%;
    overflow-y: auto;
    position: relative;
    -webkit-overflow-scrolling: touch;
  }

  .pull-indicator {
    position: absolute;
    top: -60px;
    left: 0;
    right: 0;
    height: 60px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    color: var(--text-muted, #6b7280);
    font-size: 14px;
    z-index: 10;
    will-change: transform, opacity;
  }

  .content {
    min-height: 100%;
    will-change: transform;
  }

  .spinner {
    width: 20px;
    height: 20px;
    border: 2px solid var(--border, #e5e7eb);
    border-top-color: var(--primary, #3b82f6);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  /* Dark mode support */
  @media (prefers-color-scheme: dark) {
    .pull-indicator {
      color: #9ca3af;
    }

    .spinner {
      border-color: #4b5563;
      border-top-color: #3b82f6;
    }
  }
</style>
