<script lang="ts">
  import Modal from './Modal.svelte';

  interface Props {
    title: string;
    message: string;
    /** A second line for facts, such as how much would be lost. */
    details?: string;
    confirmText: string;
    cancelText: string;
    onConfirm: (password: string) => void;
    onCancel: () => void;
    danger?: boolean;
    /** Keeps the confirm button disabled until `value` has been typed. */
    typeToConfirm?: { value: string; label: string };
    /** Asks for the account password, which `onConfirm` receives. */
    passwordLabel?: string;
    /** The action is running: nothing can be changed or dismissed. */
    busy?: boolean;
    error?: string | null;
  }

  let {
    title,
    message,
    details,
    confirmText,
    cancelText,
    onConfirm,
    onCancel,
    danger = false,
    typeToConfirm,
    passwordLabel,
    busy = false,
    error = null,
  }: Props = $props();

  const id = $props.id();
  const titleId = `${id}-title`;
  const typedId = `${id}-typed`;
  const passwordId = `${id}-password`;

  let typed = $state('');
  let password = $state('');

  const hasFields = $derived(!!typeToConfirm || !!passwordLabel);
  const canConfirm = $derived(
    !busy &&
      (!typeToConfirm || typed.trim() === typeToConfirm.value) &&
      (!passwordLabel || password !== ''),
  );

  function handleSubmit(event: Event) {
    event.preventDefault();
    if (canConfirm) {
      onConfirm(password);
    }
  }
</script>

<Modal labelledBy={titleId} onClose={onCancel} locked={busy} panelClass="max-w-md w-full mx-4">
  <form onsubmit={handleSubmit}>
    <div class="p-6">
      <h3 id={titleId} class="text-lg font-semibold mb-2" class:text-gray-900={!danger} class:text-red-600={danger}>{title}</h3>
      <p class="text-gray-600">{message}</p>
      {#if details}
        <p class="text-gray-900 font-medium mt-3">{details}</p>
      {/if}

      {#if error}
        <div role="alert" class="mt-4 bg-red-50 border border-red-200 text-red-700 px-3 py-2 rounded text-sm">
          {error}
        </div>
      {/if}

      {#if typeToConfirm}
        <div class="mt-4">
          <label for={typedId} class="block text-sm font-medium text-gray-700 mb-1">{typeToConfirm.label}</label>
          <input
            id={typedId}
            type="text"
            data-initial-focus
            autocomplete="off"
            autocapitalize="off"
            spellcheck="false"
            bind:value={typed}
            disabled={busy}
            placeholder={typeToConfirm.value}
            class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-red-500 disabled:bg-gray-100"
          />
        </div>
      {/if}

      {#if passwordLabel}
        <div class="mt-4">
          <label for={passwordId} class="block text-sm font-medium text-gray-700 mb-1">{passwordLabel}</label>
          <input
            id={passwordId}
            type="password"
            data-initial-focus={typeToConfirm ? undefined : true}
            autocomplete="current-password"
            bind:value={password}
            disabled={busy}
            class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-red-500 disabled:bg-gray-100"
          />
        </div>
      {/if}
    </div>

    <div class="bg-gray-50 px-6 py-4 flex justify-end space-x-3 rounded-b-lg">
      <!-- A dangerous action starts on Cancel, so a stray Enter can't confirm it -->
      <button
        type="button"
        onclick={onCancel}
        disabled={busy}
        data-initial-focus={danger && !hasFields ? true : undefined}
        class="px-4 py-2 bg-gray-200 text-gray-700 rounded-md hover:bg-gray-300 transition-colors disabled:opacity-50"
      >
        {cancelText}
      </button>
      <button
        type="submit"
        disabled={!canConfirm}
        data-initial-focus={!danger && !hasFields ? true : undefined}
        class="px-4 py-2 text-white rounded-md transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        class:bg-blue-600={!danger}
        class:hover:bg-blue-700={!danger}
        class:bg-red-600={danger}
        class:hover:bg-red-700={danger}
      >
        {confirmText}
      </button>
    </div>
  </form>
</Modal>
