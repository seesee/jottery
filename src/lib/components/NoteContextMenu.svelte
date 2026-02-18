<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { onMount, onDestroy } from 'svelte';
  import type { DecryptedNote } from '../types';

  export let show: boolean = false;
  export let x: number = 0;
  export let y: number = 0;
  export let note: DecryptedNote;
  export let onClose: () => void;
  export let onAction: (action: string, note: DecryptedNote) => void;

  let menuEl: HTMLElement;

  function handleClickOutside(event: MouseEvent) {
    if (menuEl && !menuEl.contains(event.target as Node)) {
      onClose();
    }
  }

  function handleKeyDown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      onClose();
    }
  }

  function handleAction(action: string) {
    onAction(action, note);
    onClose();
  }

  // Adjust position to keep menu within viewport
  function adjustPosition(el: HTMLElement) {
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const vw = window.innerWidth;
    const vh = window.innerHeight;

    if (rect.right > vw) {
      x = Math.max(8, vw - rect.width - 8);
    }
    if (rect.bottom > vh) {
      y = Math.max(8, vh - rect.height - 8);
    }
  }

  onMount(() => {
    document.addEventListener('mousedown', handleClickOutside);
    document.addEventListener('keydown', handleKeyDown, true);
    // Adjust on next frame after render
    requestAnimationFrame(() => {
      if (menuEl) adjustPosition(menuEl);
    });
  });

  onDestroy(() => {
    document.removeEventListener('mousedown', handleClickOutside);
    document.removeEventListener('keydown', handleKeyDown, true);
  });
</script>

{#if show}
  <div
    bind:this={menuEl}
    class="fixed z-[60] bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-600 rounded-lg shadow-xl py-1 min-w-[180px]"
    style="left: {x}px; top: {y}px;"
    role="menu"
    aria-label={$_('contextMenu.title')}
  >
    <!-- Open -->
    <button
      on:click={() => handleAction('open')}
      class="w-full text-left px-4 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-3"
      role="menuitem"
    >
      <svg class="w-4 h-4 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
      </svg>
      {$_('contextMenu.open')}
    </button>

    <div class="border-t border-gray-100 dark:border-gray-700 my-1"></div>

    <!-- Pin/Unpin -->
    <button
      on:click={() => handleAction('togglePin')}
      class="w-full text-left px-4 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-3"
      role="menuitem"
    >
      <span class="w-4 h-4 flex items-center justify-center text-gray-400">{note.pinned ? '⭐' : '☆'}</span>
      {note.pinned ? $_('contextMenu.unpin') : $_('contextMenu.pin')}
    </button>

    <!-- Duplicate -->
    <button
      on:click={() => handleAction('duplicate')}
      class="w-full text-left px-4 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-3"
      role="menuitem"
    >
      <svg class="w-4 h-4 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
      </svg>
      {$_('contextMenu.duplicate')}
    </button>

    <!-- Set Colour -->
    <button
      on:click={() => handleAction('setColor')}
      class="w-full text-left px-4 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-3"
      role="menuitem"
    >
      <svg class="w-4 h-4 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 21a4 4 0 01-4-4V5a2 2 0 012-2h4a2 2 0 012 2v12a4 4 0 01-4 4zm0 0h12a2 2 0 002-2v-4a2 2 0 00-2-2h-2.343M11 7.343l1.657-1.657a2 2 0 012.828 0l2.829 2.829a2 2 0 010 2.828l-8.486 8.485M7 17h.01" />
      </svg>
      {$_('contextMenu.setColour')}
    </button>

    <div class="border-t border-gray-100 dark:border-gray-700 my-1"></div>

    <!-- Archive -->
    <button
      on:click={() => handleAction('archive')}
      class="w-full text-left px-4 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-3"
      role="menuitem"
    >
      <svg class="w-4 h-4 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4" />
      </svg>
      {note.archived ? $_('contextMenu.unarchive') : $_('contextMenu.archive')}
    </button>

    <!-- Delete (not for pinned or locked notes) -->
    {#if !note.pinned && !note.locked}
      <button
        on:click={() => handleAction('delete')}
        class="w-full text-left px-4 py-2 text-sm text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 flex items-center gap-3"
        role="menuitem"
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
        </svg>
        {$_('contextMenu.delete')}
      </button>
    {/if}
  </div>
{/if}
