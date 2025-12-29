<script lang="ts">
  import { auth } from './lib/auth.svelte';
  import Login from './pages/Login.svelte';
  import Dashboard from './pages/Dashboard.svelte';
  import Users from './pages/Users.svelte';
  import Settings from './pages/Settings.svelte';
  import Toast from './components/Toast.svelte';
  import ConfirmModal from './components/ConfirmModal.svelte';

  type Page = 'dashboard' | 'users' | 'settings';
  let currentPage = $state<Page>('dashboard');
  let showLogoutConfirm = $state(false);

  function navigateTo(page: Page) {
    currentPage = page;
  }

  function handleLogout() {
    showLogoutConfirm = true;
  }

  function confirmLogout() {
    auth.logout();
    showLogoutConfirm = false;
  }
</script>

{#if !auth.isAuthenticated}
  <Login />
{:else}
  <div class="min-h-screen flex">
    <!-- Sidebar -->
    <aside class="w-64 bg-gray-800 text-white">
      <div class="p-6">
        <h1 class="text-2xl font-bold">Jottery Admin</h1>
        {#if auth.user}
          <p class="text-sm text-gray-400 mt-2">{auth.user.email}</p>
        {/if}
      </div>

      <nav class="mt-6">
        <button
          onclick={() => navigateTo('dashboard')}
          class="w-full text-left px-6 py-3 hover:bg-gray-700 transition-colors {currentPage === 'dashboard' ? 'bg-gray-700 border-l-4 border-blue-500' : ''}"
        >
          <div class="flex items-center space-x-3">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6" />
            </svg>
            <span>Dashboard</span>
          </div>
        </button>

        <button
          onclick={() => navigateTo('users')}
          class="w-full text-left px-6 py-3 hover:bg-gray-700 transition-colors {currentPage === 'users' ? 'bg-gray-700 border-l-4 border-blue-500' : ''}"
        >
          <div class="flex items-center space-x-3">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197M13 7a4 4 0 11-8 0 4 4 0 018 0z" />
            </svg>
            <span>Users</span>
          </div>
        </button>

        <button
          onclick={() => navigateTo('settings')}
          class="w-full text-left px-6 py-3 hover:bg-gray-700 transition-colors {currentPage === 'settings' ? 'bg-gray-700 border-l-4 border-blue-500' : ''}"
        >
          <div class="flex items-center space-x-3">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
            </svg>
            <span>Settings</span>
          </div>
        </button>
      </nav>

      <div class="absolute bottom-0 w-64 p-6">
        <button
          onclick={handleLogout}
          class="w-full text-left px-4 py-2 bg-gray-700 hover:bg-gray-600 rounded transition-colors flex items-center space-x-3"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
          </svg>
          <span>Log out</span>
        </button>
      </div>
    </aside>

    <!-- Main Content -->
    <main class="flex-1 overflow-auto">
      <div class="max-w-7xl mx-auto p-8">
        {#if currentPage === 'dashboard'}
          <Dashboard />
        {:else if currentPage === 'users'}
          <Users />
        {:else if currentPage === 'settings'}
          <Settings />
        {/if}
      </div>
    </main>
  </div>

  {#if showLogoutConfirm}
    <ConfirmModal
      title="Confirm Logout"
      message="Are you sure you want to log out?"
      confirmText="Log out"
      cancelText="Cancel"
      onConfirm={confirmLogout}
      onCancel={() => showLogoutConfirm = false}
    />
  {/if}
{/if}

<Toast />
