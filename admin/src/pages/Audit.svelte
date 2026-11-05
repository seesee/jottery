<script lang="ts">
  import { api } from '../lib/api';
  import type { AuditEntry } from '../lib/api';
  import { describeError } from '../lib/apiErrors';
  import { AUDIT_ACTIONS } from '../lib/auditActions';
  import { onMount } from 'svelte';
  import { _, locale } from 'svelte-i18n';

  const PAGE_SIZE = 50;

  let entries = $state<AuditEntry[]>([]);
  let total = $state(0);
  let offset = $state(0);
  let action = $state('');
  let loading = $state(true);
  let error = $state<string | null>(null);
  let emails = $state(new Map<string, string>());

  async function load() {
    loading = true;
    error = null;
    try {
      const response = await api.getAuditLog({
        limit: PAGE_SIZE,
        offset,
        ...(action ? { action } : {}),
      });
      entries = response.logs;
      total = response.total;
    } catch (err) {
      error = describeError(err, $_, 'audit.failedToLoad');
    } finally {
      loading = false;
    }
  }

  // Only a nicety: the log still reads without it, with ids in place of addresses
  async function loadEmails() {
    try {
      const { users } = await api.listUsers();
      emails = new Map(users.map((user) => [user.id, user.email]));
    } catch (err) {
      console.warn('Could not load user addresses:', err);
    }
  }

  function changePage(change: number) {
    offset = Math.max(0, offset + change);
    load();
  }

  function handleFilterChange(event: Event) {
    action = (event.target as HTMLSelectElement).value;
    offset = 0;
    load();
  }

  function actionName(name: string): string {
    const key = AUDIT_ACTIONS[name];
    return key ? $_(key) : name;
  }

  onMount(() => {
    load();
    loadEmails();
  });

  // As on the Users page, rows stack into cards below lg
  const headerCell = 'px-3 xl:px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider';
  const cell = 'block lg:table-cell py-1 lg:px-3 xl:px-6 lg:py-4 text-sm align-top';
</script>

{#snippet account(id: string)}
  <!-- A deleted account is no longer in the list, so all that's left is its id -->
  {#if emails.has(id)}
    <span class="break-all">{emails.get(id)}</span>
  {:else}
    <span class="font-mono text-xs" title={id}>{id.slice(0, 8)}…</span>
  {/if}
{/snippet}

<div class="space-y-6">
  <div class="flex justify-between items-center gap-4">
    <div>
      <h1 class="text-3xl font-bold text-gray-900">{$_('audit.title')}</h1>
      <p class="text-sm text-gray-600 mt-1">{$_('audit.description')}</p>
    </div>
    <button
      onclick={load}
      class="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:bg-gray-400"
      disabled={loading}
    >
      {loading ? $_('common.refreshing') : $_('common.refresh')}
    </button>
  </div>

  {#if error}
    <div role="alert" class="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
      {error}
    </div>
  {/if}

  <div class="flex items-center gap-3">
    <label for="audit-action" class="text-sm font-medium text-gray-700">{$_('audit.filterLabel')}</label>
    <select
      id="audit-action"
      value={action}
      onchange={handleFilterChange}
      class="px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 bg-white text-sm"
    >
      <option value="">{$_('audit.allActions')}</option>
      {#each Object.keys(AUDIT_ACTIONS) as name (name)}
        <option value={name}>{actionName(name)}</option>
      {/each}
    </select>
  </div>

  {#if loading && entries.length === 0}
    <div class="flex justify-center items-center py-12">
      <div class="text-gray-500">{$_('audit.loading')}</div>
    </div>
  {:else if entries.length === 0 && !error}
    <div class="bg-white rounded-lg shadow p-8 text-center text-gray-500">
      {$_('audit.empty')}
    </div>
  {:else if entries.length > 0}
    <div class="bg-white rounded-lg shadow overflow-x-auto">
      <table class="block lg:table min-w-full divide-y divide-gray-200">
        <thead class="hidden lg:table-header-group bg-gray-50">
          <tr>
            <th class={headerCell}>{$_('audit.table.time')}</th>
            <th class={headerCell}>{$_('audit.table.action')}</th>
            <th class={headerCell}>{$_('audit.table.admin')}</th>
            <th class={headerCell}>{$_('audit.table.target')}</th>
            <th class={headerCell}>{$_('audit.table.address')}</th>
          </tr>
        </thead>
        <tbody class="block lg:table-row-group bg-white divide-y divide-gray-200">
          {#each entries as entry (entry.id)}
            <tr class="block lg:table-row p-4 lg:p-0">
              <td class="{cell} text-gray-500 lg:whitespace-nowrap">
                {new Date(entry.createdAt).toLocaleString($locale ?? undefined)}
              </td>
              <td class={cell}>
                <span class="font-medium text-gray-900">{actionName(entry.action)}</span>
                {#if entry.metadata}
                  <code class="block text-xs text-gray-500 break-all mt-1">{JSON.stringify(entry.metadata)}</code>
                {/if}
              </td>
              <td class="{cell} text-gray-700">
                <span class="lg:hidden text-gray-500">{$_('audit.table.admin')}:</span>
                {#if entry.adminUserId}
                  {@render account(entry.adminUserId)}
                {:else}
                  {$_('audit.commandLine')}
                {/if}
              </td>
              <td class="{cell} text-gray-700">
                {#if entry.targetUserId || entry.targetResourceId}
                  <span class="lg:hidden text-gray-500">{$_('audit.table.target')}:</span>
                {/if}
                {#if entry.targetUserId}
                  {@render account(entry.targetUserId)}
                {:else if entry.targetResourceId}
                  {@render account(entry.targetResourceId)}
                {/if}
              </td>
              <td class="{cell} text-gray-500" title={entry.userAgent ?? undefined}>
                {#if entry.ipAddress}
                  <span class="lg:hidden">{$_('audit.table.address')}:</span>
                  {entry.ipAddress}
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>

    <div class="flex items-center justify-between">
      <p class="text-sm text-gray-600">
        {$_('audit.range', { values: { from: offset + 1, to: offset + entries.length, total } })}
      </p>
      <div class="flex gap-2">
        <button
          onclick={() => changePage(-PAGE_SIZE)}
          disabled={loading || offset === 0}
          class="px-3 py-1.5 text-sm bg-gray-200 text-gray-700 rounded-md hover:bg-gray-300 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {$_('audit.previous')}
        </button>
        <button
          onclick={() => changePage(PAGE_SIZE)}
          disabled={loading || offset + entries.length >= total}
          class="px-3 py-1.5 text-sm bg-gray-200 text-gray-700 rounded-md hover:bg-gray-300 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {$_('audit.next')}
        </button>
      </div>
    </div>
  {/if}
</div>
