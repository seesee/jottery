<script lang="ts">
  import { userAuth } from './lib/userAuth.svelte';
  import Login from './pages/user/Login.svelte';
  import Account from './pages/user/Account.svelte';
  import UserSettings from './pages/user/UserSettings.svelte';
  import Toast from './components/Toast.svelte';
  import ConfirmModal from './components/ConfirmModal.svelte';
  import { _ } from 'svelte-i18n';

  type Tab = 'account' | 'settings';
  let currentTab = $state<Tab>('account');
  let showLogoutConfirm = $state(false);

  function navigateTo(tab: Tab) {
    currentTab = tab;
  }

  function handleLogout() {
    showLogoutConfirm = true;
  }

  async function confirmLogout() {
    await userAuth.logout();
    showLogoutConfirm = false;
  }
</script>

{#if !userAuth.isAuthenticated}
  <Login />
{:else}
  <div class="min-h-screen bg-gray-100">
    <!-- Header -->
    <header class="bg-white shadow">
      <div class="max-w-4xl mx-auto px-4 py-4 flex items-center justify-between">
        <div>
          <h1 class="text-2xl font-bold text-gray-900">{$_('userPortal.title')}</h1>
          {#if userAuth.user}
            <p class="text-sm text-gray-500">{userAuth.user.email}</p>
          {/if}
        </div>
        <button
          onclick={handleLogout}
          class="flex items-center space-x-2 px-4 py-2 text-gray-600 hover:text-gray-900 hover:bg-gray-100 rounded-md transition-colors"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
          </svg>
          <span>{$_('nav.logout')}</span>
        </button>
      </div>
    </header>

    <!-- Tab Navigation -->
    <div class="max-w-4xl mx-auto px-4 mt-6">
      <div class="border-b border-gray-200">
        <nav class="-mb-px flex space-x-8">
          <button
            onclick={() => navigateTo('account')}
            class="py-4 px-1 border-b-2 font-medium text-sm transition-colors {currentTab === 'account'
              ? 'border-blue-500 text-blue-600'
              : 'border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300'}"
          >
            <div class="flex items-center space-x-2">
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
              </svg>
              <span>{$_('userPortal.tabs.account')}</span>
            </div>
          </button>

          <button
            onclick={() => navigateTo('settings')}
            class="py-4 px-1 border-b-2 font-medium text-sm transition-colors {currentTab === 'settings'
              ? 'border-blue-500 text-blue-600'
              : 'border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300'}"
          >
            <div class="flex items-center space-x-2">
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
              </svg>
              <span>{$_('userPortal.tabs.settings')}</span>
            </div>
          </button>
        </nav>
      </div>
    </div>

    <!-- Main Content -->
    <main class="max-w-4xl mx-auto px-4 py-8">
      {#if currentTab === 'account'}
        <Account />
      {:else if currentTab === 'settings'}
        <UserSettings />
      {/if}
    </main>
  </div>

  {#if showLogoutConfirm}
    <ConfirmModal
      title={$_('logout.confirmTitle')}
      message={$_('logout.confirmMessage')}
      confirmText={$_('nav.logout')}
      cancelText={$_('common.cancel')}
      onConfirm={confirmLogout}
      onCancel={() => showLogoutConfirm = false}
    />
  {/if}
{/if}

<Toast />
