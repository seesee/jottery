<script lang="ts">
  /**
   * TabContainer - A reusable tab navigation component
   *
   * Handles tab bar rendering, keyboard navigation (arrow keys, Home, End),
   * and proper ARIA attributes for accessibility.
   *
   * Usage:
   * <TabContainer {tabs} bind:currentTab>
   *   {#if currentTab === 'general'}
   *     <GeneralContent />
   *   {:else if currentTab === 'advanced'}
   *     <AdvancedContent />
   *   {/if}
   * </TabContainer>
   */

  interface Tab {
    id: string;
    label: string;
  }

  export let tabs: Tab[] = [];
  export let currentTab: string = tabs[0]?.id ?? '';
  export let ariaLabel: string = 'Tabs';

  function selectTab(tabId: string) {
    currentTab = tabId;
  }

  function handleKeyDown(event: KeyboardEvent, index: number) {
    let newIndex: number | null = null;

    switch (event.key) {
      case 'ArrowLeft':
        event.preventDefault();
        newIndex = index > 0 ? index - 1 : tabs.length - 1;
        break;
      case 'ArrowRight':
        event.preventDefault();
        newIndex = index < tabs.length - 1 ? index + 1 : 0;
        break;
      case 'Home':
        event.preventDefault();
        newIndex = 0;
        break;
      case 'End':
        event.preventDefault();
        newIndex = tabs.length - 1;
        break;
    }

    if (newIndex !== null) {
      selectTab(tabs[newIndex].id);
      // Focus the new tab button after DOM updates
      requestAnimationFrame(() => {
        const container = event.currentTarget as HTMLElement;
        const tablist = container.closest('[role="tablist"]');
        const buttons = tablist?.querySelectorAll('[role="tab"]');
        (buttons?.[newIndex!] as HTMLElement)?.focus();
      });
    }
  }
</script>

<div class="flex-1 min-h-0 flex flex-col">
  <!-- Tab Navigation -->
  <div
    class="border-b border-gray-200 dark:border-gray-700 flex overflow-x-auto flex-shrink-0"
    role="tablist"
    aria-label={ariaLabel}
  >
    {#each tabs as tab, index (tab.id)}
      <button
        role="tab"
        aria-selected={currentTab === tab.id}
        aria-controls="tabpanel-{tab.id}"
        id="tab-{tab.id}"
        tabindex={currentTab === tab.id ? 0 : -1}
        on:click={() => selectTab(tab.id)}
        on:keydown={(e) => handleKeyDown(e, index)}
        class="px-4 py-3 text-sm font-medium whitespace-nowrap transition-colors {currentTab === tab.id
          ? 'text-blue-600 dark:text-blue-400 border-b-2 border-blue-600 dark:border-blue-400'
          : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200'}"
      >
        {tab.label}
      </button>
    {/each}
  </div>

  <!-- Tab Content (provided by parent via slot) -->
  <div
    class="flex-1 overflow-y-auto"
    role="tabpanel"
    id="tabpanel-{currentTab}"
    aria-labelledby="tab-{currentTab}"
  >
    <slot />
  </div>
</div>
