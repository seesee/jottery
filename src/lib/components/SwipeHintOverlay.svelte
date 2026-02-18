<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { settings } from '../stores/appStore';
  import { settingsRepository } from '../services';

  export let show: boolean = false;

  let dismissed = false;
  let autoTimeout: ReturnType<typeof setTimeout> | null = null;

  $: visible = show && !dismissed && !$settings.swipeHintsShown;

  async function dismiss() {
    dismissed = true;
    // Persist that hints have been shown
    settings.update(s => ({ ...s, swipeHintsShown: true }));
    await settingsRepository.update({ swipeHintsShown: true });
  }

  $: if (visible) {
    // Auto-dismiss after 5 seconds
    autoTimeout = setTimeout(dismiss, 5000);
  }

  $: if (!visible && autoTimeout) {
    clearTimeout(autoTimeout);
    autoTimeout = null;
  }
</script>

{#if visible}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-[70] bg-black/60 flex flex-col items-center justify-center gap-8 text-white"
    on:click={dismiss}
    on:keydown={(e) => e.key === 'Escape' && dismiss()}
  >
    <!-- Swipe left hint (delete) -->
    <div class="flex items-center gap-4 animate-slide-left">
      <div class="text-center">
        <div class="flex items-center gap-2 mb-1">
          <span class="text-2xl">👈</span>
          <svg class="w-6 h-6 animate-swipe-left" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16l-4-4m0 0l4-4m-4 4h18" />
          </svg>
        </div>
        <p class="text-sm font-medium">{$_('swipeHints.swipeLeftToDelete')}</p>
      </div>
    </div>

    <!-- Swipe right hint (multi-select) -->
    <div class="flex items-center gap-4 animate-slide-right">
      <div class="text-center">
        <div class="flex items-center gap-2 mb-1">
          <svg class="w-6 h-6 animate-swipe-right" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 8l4 4m0 0l-4 4m4-4H3" />
          </svg>
          <span class="text-2xl">👉</span>
        </div>
        <p class="text-sm font-medium">{$_('swipeHints.swipeRightToSelect')}</p>
      </div>
    </div>

    <!-- Dismiss hint -->
    <p class="text-xs text-gray-300 mt-4 animate-fade-in">{$_('swipeHints.tapToDismiss')}</p>
  </div>
{/if}

<style>
  @keyframes slide-left {
    0% { opacity: 0; transform: translateX(30px); }
    100% { opacity: 1; transform: translateX(0); }
  }

  @keyframes slide-right {
    0% { opacity: 0; transform: translateX(-30px); }
    100% { opacity: 1; transform: translateX(0); }
  }

  @keyframes swipe-left {
    0%, 100% { transform: translateX(0); }
    50% { transform: translateX(-12px); }
  }

  @keyframes swipe-right {
    0%, 100% { transform: translateX(0); }
    50% { transform: translateX(12px); }
  }

  @keyframes fade-in {
    0% { opacity: 0; }
    100% { opacity: 1; }
  }

  .animate-slide-left {
    animation: slide-left 0.5s ease-out 0.3s both;
  }

  .animate-slide-right {
    animation: slide-right 0.5s ease-out 0.6s both;
  }

  .animate-swipe-left {
    animation: swipe-left 1.5s ease-in-out 0.8s infinite;
  }

  .animate-swipe-right {
    animation: swipe-right 1.5s ease-in-out 1.1s infinite;
  }

  .animate-fade-in {
    animation: fade-in 0.5s ease-out 1s both;
  }
</style>
