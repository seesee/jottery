<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { _ } from 'svelte-i18n';

  const dispatch = createEventDispatcher<{ keydown: KeyboardEvent }>();

  export let value: string = '';
  export let id: string = '';
  export let placeholder: string = '';
  export let disabled: boolean = false;
  export let autocomplete: HTMLInputElement['autocomplete'] = 'off';
  export let className: string = '';

  let showPassword = false;
  let inputElement: HTMLInputElement;

  function toggleVisibility() {
    showPassword = !showPassword;
  }

  export function focus() {
    inputElement?.focus();
  }
</script>

<div class="relative">
  <!-- svelte-ignore a11y_autofocus -->
  <input
    {id}
    type={showPassword ? 'text' : 'password'}
    bind:value
    bind:this={inputElement}
    {placeholder}
    {disabled}
    {autocomplete}
    class="{className} pr-10"
    on:keydown={(e) => { dispatch('keydown', e); }}
    on:input
    on:focus
    on:blur
  />
  <button
    type="button"
    on:click={toggleVisibility}
    class="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 focus:outline-none"
    tabindex="-1"
    aria-label={showPassword ? $_('common.hidePassword') : $_('common.showPassword')}
  >
    {#if showPassword}
      <!-- Eye slash icon (hide) -->
      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l3.59 3.59m0 0A9.953 9.953 0 0112 5c4.478 0 8.268 2.943 9.543 7a10.025 10.025 0 01-4.132 5.411m0 0L21 21" />
      </svg>
    {:else}
      <!-- Eye icon (show) -->
      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
      </svg>
    {/if}
  </button>
</div>
