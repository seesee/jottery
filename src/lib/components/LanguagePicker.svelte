<script lang="ts">
  import { locale, _ } from 'svelte-i18n';
  import { AVAILABLE_LOCALES } from '../services/i18nService';
  import { onMount } from 'svelte';

  let isOpen = false;
  let dropdownRef: HTMLDivElement;

  // Get the current locale's display info
  $: currentLocale = AVAILABLE_LOCALES.find(l => l.code === $locale) || AVAILABLE_LOCALES[0];

  // Extract just the flag emoji from the name
  $: currentFlag = currentLocale.name.split(' ')[0];

  function toggleDropdown() {
    isOpen = !isOpen;
  }

  function selectLocale(code: string) {
    locale.set(code);
    localStorage.setItem('jottery-locale', code);
    isOpen = false;
  }

  function handleClickOutside(event: MouseEvent) {
    if (dropdownRef && !dropdownRef.contains(event.target as Node)) {
      isOpen = false;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      isOpen = false;
    }
  }

  onMount(() => {
    document.addEventListener('click', handleClickOutside);
    document.addEventListener('keydown', handleKeydown);

    return () => {
      document.removeEventListener('click', handleClickOutside);
      document.removeEventListener('keydown', handleKeydown);
    };
  });
</script>

<div class="language-picker" bind:this={dropdownRef}>
  <button
    type="button"
    class="picker-button"
    on:click={toggleDropdown}
    aria-haspopup="listbox"
    aria-expanded={isOpen}
    aria-label={$_('landing.languagePicker.label', { default: 'Select language' })}
  >
    <svg
      class="globe-icon"
      xmlns="http://www.w3.org/2000/svg"
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <circle cx="12" cy="12" r="10"></circle>
      <line x1="2" y1="12" x2="22" y2="12"></line>
      <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"></path>
    </svg>
    <span class="current-locale">{currentFlag}</span>
    <svg
      class="chevron-icon"
      class:open={isOpen}
      xmlns="http://www.w3.org/2000/svg"
      width="14"
      height="14"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <polyline points="6 9 12 15 18 9"></polyline>
    </svg>
  </button>

  {#if isOpen}
    <ul
      class="dropdown-menu"
      role="listbox"
      aria-label={$_('landing.languagePicker.label', { default: 'Select language' })}
    >
      {#each AVAILABLE_LOCALES as loc}
        <li>
          <button
            type="button"
            class="locale-option"
            class:selected={loc.code === $locale}
            on:click={() => selectLocale(loc.code)}
            role="option"
            aria-selected={loc.code === $locale}
          >
            <span class="locale-name">{loc.name}</span>
            {#if loc.code === $locale}
              <svg
                class="check-icon"
                xmlns="http://www.w3.org/2000/svg"
                width="16"
                height="16"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <polyline points="20 6 9 17 4 12"></polyline>
              </svg>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .language-picker {
    position: relative;
    display: inline-block;
  }

  .picker-button {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    padding: 0.5rem 0.75rem;
    background: transparent;
    border: 1px solid #e5e7eb;
    border-radius: 0.5rem;
    cursor: pointer;
    color: #374151;
    font-size: 0.875rem;
    transition: all 0.2s;
  }

  .picker-button:hover {
    background: #f9fafb;
    border-color: #d1d5db;
  }

  :global(.dark) .picker-button {
    border-color: #374151;
    color: #d1d5db;
  }

  :global(.dark) .picker-button:hover {
    background: #1f2937;
    border-color: #4b5563;
  }

  .globe-icon {
    flex-shrink: 0;
    opacity: 0.7;
  }

  .current-locale {
    font-size: 1.125rem;
    line-height: 1;
  }

  .chevron-icon {
    flex-shrink: 0;
    opacity: 0.5;
    transition: transform 0.2s;
  }

  .chevron-icon.open {
    transform: rotate(180deg);
  }

  .dropdown-menu {
    position: absolute;
    top: calc(100% + 0.5rem);
    right: 0;
    min-width: 180px;
    max-height: 320px;
    overflow-y: auto;
    background: #ffffff;
    border: 1px solid #e5e7eb;
    border-radius: 0.5rem;
    box-shadow: 0 10px 15px -3px rgba(0, 0, 0, 0.1), 0 4px 6px -2px rgba(0, 0, 0, 0.05);
    list-style: none;
    margin: 0;
    padding: 0.25rem;
    z-index: 100;
    animation: dropdown-appear 0.15s ease-out;
  }

  @keyframes dropdown-appear {
    from {
      opacity: 0;
      transform: translateY(-0.5rem);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  :global(.dark) .dropdown-menu {
    background: #1f2937;
    border-color: #374151;
    box-shadow: 0 10px 15px -3px rgba(0, 0, 0, 0.3), 0 4px 6px -2px rgba(0, 0, 0, 0.2);
  }

  .locale-option {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 0.5rem 0.75rem;
    background: transparent;
    border: none;
    border-radius: 0.375rem;
    cursor: pointer;
    color: #374151;
    font-size: 0.875rem;
    text-align: left;
    transition: background 0.15s;
  }

  .locale-option:hover {
    background: #f3f4f6;
  }

  .locale-option.selected {
    background: #eff6ff;
    color: #2563eb;
  }

  :global(.dark) .locale-option {
    color: #d1d5db;
  }

  :global(.dark) .locale-option:hover {
    background: #374151;
  }

  :global(.dark) .locale-option.selected {
    background: #1e3a5f;
    color: #60a5fa;
  }

  .locale-name {
    flex: 1;
  }

  .check-icon {
    flex-shrink: 0;
    color: currentColor;
  }
</style>
