<script lang="ts">
  import { api } from '../lib/api';
  import type { StatsResponse } from '../lib/api';
  import { onMount } from 'svelte';

  let stats = $state<StatsResponse | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  async function loadStats() {
    try {
      loading = true;
      error = null;
      stats = await api.getStats();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load statistics';
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    loadStats();
  });
</script>

<div class="space-y-6">
  <div class="flex justify-between items-center">
    <h1 class="text-3xl font-bold text-gray-900">Dashboard</h1>
    <button
      onclick={loadStats}
      class="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:bg-gray-400"
      disabled={loading}
    >
      {loading ? 'Refreshing...' : 'Refresh'}
    </button>
  </div>

  {#if error}
    <div class="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
      {error}
    </div>
  {/if}

  {#if loading && !stats}
    <div class="flex justify-center items-center py-12">
      <div class="text-gray-500">Loading statistics...</div>
    </div>
  {:else if stats}
    <!-- Statistics Grid -->
    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
      <!-- Total Users -->
      <div class="bg-white rounded-lg shadow p-6">
        <div class="flex items-center justify-between">
          <div>
            <p class="text-sm font-medium text-gray-600">Total Users</p>
            <p class="text-3xl font-bold text-gray-900 mt-2">{stats.users.total}</p>
          </div>
          <div class="bg-blue-100 rounded-full p-3">
            <svg class="w-6 h-6 text-blue-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197M13 7a4 4 0 11-8 0 4 4 0 018 0z" />
            </svg>
          </div>
        </div>
        <div class="mt-4 text-sm text-gray-600">
          <span class="text-green-600 font-medium">{stats.users.approved}</span> approved,
          <span class="text-yellow-600 font-medium">{stats.users.pending}</span> pending
        </div>
      </div>

      <!-- Active Devices -->
      <div class="bg-white rounded-lg shadow p-6">
        <div class="flex items-center justify-between">
          <div>
            <p class="text-sm font-medium text-gray-600">Active Devices</p>
            <p class="text-3xl font-bold text-gray-900 mt-2">{stats.devices.active}</p>
          </div>
          <div class="bg-green-100 rounded-full p-3">
            <svg class="w-6 h-6 text-green-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" />
            </svg>
          </div>
        </div>
        <div class="mt-4 text-sm text-gray-600">
          {stats.devices.total} total devices
        </div>
      </div>

      <!-- Total Notes -->
      <div class="bg-white rounded-lg shadow p-6">
        <div class="flex items-center justify-between">
          <div>
            <p class="text-sm font-medium text-gray-600">Total Notes</p>
            <p class="text-3xl font-bold text-gray-900 mt-2">{stats.notes.total}</p>
          </div>
          <div class="bg-purple-100 rounded-full p-3">
            <svg class="w-6 h-6 text-purple-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
            </svg>
          </div>
        </div>
        <div class="mt-4 text-sm text-gray-600">
          Across all users
        </div>
      </div>

      <!-- Storage Usage -->
      <div class="bg-white rounded-lg shadow p-6">
        <div class="flex items-center justify-between">
          <div>
            <p class="text-sm font-medium text-gray-600">Storage</p>
            <p class="text-3xl font-bold text-gray-900 mt-2">
              {(stats.storage.totalBytes / 1024 / 1024).toFixed(1)} MB
            </p>
          </div>
          <div class="bg-orange-100 rounded-full p-3">
            <svg class="w-6 h-6 text-orange-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4" />
            </svg>
          </div>
        </div>
        <div class="mt-4 text-sm text-gray-600">
          {(stats.storage.quotaBytes / 1024 / 1024).toFixed(0)} MB quota
        </div>
      </div>
    </div>

    <!-- Recent Activity Section (Placeholder) -->
    <div class="bg-white rounded-lg shadow p-6">
      <h2 class="text-xl font-semibold text-gray-900 mb-4">Server Information</h2>
      <div class="space-y-3 text-sm text-gray-600">
        <div class="flex justify-between">
          <span>Server Version:</span>
          <span class="font-medium text-gray-900">v0.6.0</span>
        </div>
        <div class="flex justify-between">
          <span>Active Users:</span>
          <span class="font-medium text-gray-900">{stats.users.active}</span>
        </div>
        <div class="flex justify-between">
          <span>Pending Approvals:</span>
          <span class="font-medium text-gray-900">{stats.users.pending}</span>
        </div>
      </div>
    </div>
  {/if}
</div>
