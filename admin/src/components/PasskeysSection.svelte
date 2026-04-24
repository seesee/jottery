<script lang="ts">
  // Passkey enrolment and management for the user portal.
  //
  // On load: fetches the current list. 404 → server hasn't configured
  // WebAuthn, hide the whole section (rather than showing a broken UI).
  // On errors: show an inline message and a retry button; don't block
  // other parts of the settings page.

  import { userApi, ApiError, type Passkey } from '../lib/userApi';
  import { toast } from '../lib/toast.svelte';
  import ConfirmModal from './ConfirmModal.svelte';
  import { _ } from 'svelte-i18n';
  import { onMount } from 'svelte';

  let passkeys = $state<Passkey[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let supported = $state<boolean>(true); // Server-side support; hides the section if 404.
  let enrolling = $state(false);
  let nickname = $state('');
  let deleteTarget = $state<Passkey | null>(null);

  // The browser must also support WebAuthn. Chrome/Safari/Firefox modern
  // versions and any platform that has a credential store do. If it doesn't,
  // we still show the section so the user understands the feature exists,
  // but the enrol button is disabled with a hint.
  const browserSupportsWebAuthn = typeof window !== 'undefined'
    && !!(window.PublicKeyCredential);

  onMount(async () => {
    await refresh();
  });

  async function refresh() {
    loading = true;
    loadError = null;
    try {
      passkeys = await userApi.listPasskeys();
    } catch (err) {
      if (err instanceof ApiError && err.status === 404) {
        supported = false;
        return;
      }
      loadError = $_('userPortal.settings.passkeys.loadError');
    } finally {
      loading = false;
    }
  }

  async function handleEnroll(e: Event) {
    e.preventDefault();
    if (!browserSupportsWebAuthn) return;
    enrolling = true;
    try {
      const newPasskey = await userApi.registerPasskey(nickname.trim() || null);
      passkeys = [newPasskey, ...passkeys];
      nickname = '';
      toast.success($_('userPortal.settings.passkeys.enrolled'));
    } catch (err) {
      if (err instanceof DOMException && (err.name === 'NotAllowedError' || err.name === 'AbortError')) {
        // User cancelled the browser prompt — quietly no-op.
        return;
      }
      if (err instanceof ApiError && err.status === 404) {
        supported = false;
        return;
      }
      toast.error($_('userPortal.settings.passkeys.enrollFailed'));
      console.error('Passkey enrol failed:', err);
    } finally {
      enrolling = false;
    }
  }

  async function handleDelete(p: Passkey) {
    try {
      await userApi.deletePasskey(p.id);
      passkeys = passkeys.filter((x) => x.id !== p.id);
      toast.success($_('userPortal.settings.passkeys.deleted'));
    } catch (err) {
      toast.error($_('userPortal.settings.passkeys.deleteFailed'));
      console.error('Passkey delete failed:', err);
    } finally {
      deleteTarget = null;
    }
  }

  function formatDate(iso: string | null): string {
    if (!iso) return '—';
    return new Date(iso).toLocaleString();
  }
</script>

{#if supported}
  <div class="bg-white rounded-lg shadow p-6">
    <h3 class="text-lg font-semibold text-gray-900 mb-2">
      {$_('userPortal.settings.passkeys.title')}
    </h3>
    <p class="text-sm text-gray-600 mb-4">
      {$_('userPortal.settings.passkeys.description')}
    </p>

    <!-- Enrolment form -->
    <form onsubmit={handleEnroll} class="flex items-end gap-3 mb-6 max-w-md">
      <div class="flex-1">
        <label for="passkey-nickname" class="block text-sm font-medium text-gray-700 mb-1">
          {$_('userPortal.settings.passkeys.nicknameLabel')}
        </label>
        <input
          id="passkey-nickname"
          type="text"
          bind:value={nickname}
          disabled={enrolling || !browserSupportsWebAuthn}
          placeholder={$_('userPortal.settings.passkeys.nicknamePlaceholder')}
          class="w-full px-3 py-2 border border-gray-300 rounded-md text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:bg-gray-100"
        />
      </div>
      <button
        type="submit"
        disabled={enrolling || !browserSupportsWebAuthn}
        class="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors text-sm"
      >
        {enrolling ? $_('userPortal.settings.passkeys.enrolling') : $_('userPortal.settings.passkeys.enrollButton')}
      </button>
    </form>

    {#if !browserSupportsWebAuthn}
      <div class="bg-yellow-50 border border-yellow-200 text-yellow-800 px-3 py-2 rounded text-sm mb-4">
        {$_('userPortal.settings.passkeys.browserUnsupported')}
      </div>
    {/if}

    <!-- List -->
    {#if loading}
      <p class="text-sm text-gray-500">{$_('common.loading')}</p>
    {:else if loadError}
      <div class="bg-red-50 border border-red-200 text-red-700 px-3 py-2 rounded text-sm">
        {loadError}
        <button onclick={refresh} class="ml-2 underline">
          {$_('common.retry')}
        </button>
      </div>
    {:else if passkeys.length === 0}
      <p class="text-sm text-gray-500">{$_('userPortal.settings.passkeys.empty')}</p>
    {:else}
      <ul class="divide-y border border-gray-200 rounded-md">
        {#each passkeys as p (p.id)}
          <li class="flex items-center justify-between p-3">
            <div>
              <div class="font-medium text-gray-900 text-sm">
                {p.nickname || $_('userPortal.settings.passkeys.unnamed')}
              </div>
              <div class="text-xs text-gray-500">
                {$_('userPortal.settings.passkeys.addedOn', { values: { date: formatDate(p.createdAt) } })}
                {#if p.lastUsedAt}
                  · {$_('userPortal.settings.passkeys.lastUsed', { values: { date: formatDate(p.lastUsedAt) } })}
                {/if}
              </div>
            </div>
            <button
              onclick={() => deleteTarget = p}
              class="text-sm text-red-600 hover:text-red-800 hover:underline"
            >
              {$_('common.remove')}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
{/if}

{#if deleteTarget}
  <ConfirmModal
    title={$_('userPortal.settings.passkeys.deleteConfirmTitle')}
    message={$_('userPortal.settings.passkeys.deleteConfirmMessage', { values: { name: deleteTarget.nickname || $_('userPortal.settings.passkeys.unnamed') } })}
    confirmText={$_('common.remove')}
    cancelText={$_('common.cancel')}
    onConfirm={() => handleDelete(deleteTarget!)}
    onCancel={() => deleteTarget = null}
    danger={true}
  />
{/if}
