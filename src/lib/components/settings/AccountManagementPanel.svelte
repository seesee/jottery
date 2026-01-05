<script lang="ts">
  export let showAccountManagement: boolean;
  export let accountEmail: string;
  export let accountPassword: string;
  export let loggingIn: boolean;
  export let userSession: { sessionId: string; email: string; isAdmin: boolean } | null;
  export let accountInfo: {
    email: string;
    noteCount: number;
    attachmentCount: number;
    storageUsedBytes: number;
    storageQuotaMb: number;
    createdAt: string;
    lastSyncAt: string | null;
  } | null;
  export let loadingAccountInfo: boolean;
  export let deletingNotes: boolean;

  export let onAccountLogin: () => void;
  export let onAccountLogout: () => void;
  export let onShowDeleteServerNotesConfirm: () => void;
</script>

<div class="bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg p-4 space-y-3">
  <button
    on:click={() => showAccountManagement = !showAccountManagement}
    class="w-full flex items-center justify-between text-sm font-medium text-gray-900 dark:text-white"
  >
    <span>👤 Manage Your Account</span>
    <span class="transform transition-transform {showAccountManagement ? 'rotate-180' : ''}">
      ▼
    </span>
  </button>

  {#if showAccountManagement}
    {#if !userSession}
                  <div class="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4 space-y-3">
                    <p class="text-sm text-blue-900 dark:text-blue-100 mb-3">
                      Log in to view your account information and manage your synced notes.
                    </p>

                    <div>
                      <label for="account-email" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                        Email Address
                      </label>
                      <input
                        id="account-email"
                        type="email"
                        bind:value={accountEmail}
                        placeholder="you@example.com"
                        class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                      />
                    </div>

                    <div>
                      <label for="account-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                        Password
                      </label>
                      <input
                        id="account-password"
                        type="password"
                        bind:value={accountPassword}
                        placeholder="••••••••••••"
                        class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                      />
                    </div>

                    <button
                      on:click={onAccountLogin}
                      disabled={!accountEmail || !accountPassword || loggingIn}
                      class="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white text-sm font-medium rounded-md transition-colors"
                    >
                      {loggingIn ? 'Logging in...' : 'Log In to View Account'}
                    </button>
                  </div>
    {:else}
                  <div class="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg p-4 space-y-4">
                    <div class="flex items-center justify-between">
                      <div>
                        <div class="text-sm font-medium text-green-900 dark:text-green-100">
                          Logged in as: {userSession.email}
                        </div>
                        {#if userSession.isAdmin}
                          <div class="text-xs text-green-700 dark:text-green-300 mt-1">
                            ⭐ Admin Account
                          </div>
                        {/if}
                      </div>
                      <button
                        on:click={onAccountLogout}
                        class="text-xs text-green-700 dark:text-green-300 hover:text-green-900 dark:hover:text-green-100"
                      >
                        Log Out
                      </button>
                    </div>

                    {#if loadingAccountInfo}
                      <div class="text-center py-4">
                        <div class="text-sm text-gray-600 dark:text-gray-400">
                          Loading account information...
                        </div>
                      </div>
                    {:else if accountInfo}
                      <!-- Account Statistics -->
                      <div class="grid grid-cols-2 gap-3">
                        <div class="bg-white dark:bg-gray-800 rounded-lg p-3 border border-green-200 dark:border-green-700">
                          <div class="text-xs text-gray-600 dark:text-gray-400">
                            Synced Notes
                          </div>
                          <div class="text-lg font-semibold text-gray-900 dark:text-white">
                            {accountInfo.noteCount.toLocaleString()}
                          </div>
                        </div>

                        <div class="bg-white dark:bg-gray-800 rounded-lg p-3 border border-green-200 dark:border-green-700">
                          <div class="text-xs text-gray-600 dark:text-gray-400">
                            Attachments
                          </div>
                          <div class="text-lg font-semibold text-gray-900 dark:text-white">
                            {accountInfo.attachmentCount.toLocaleString()}
                          </div>
                        </div>

                        <div class="bg-white dark:bg-gray-800 rounded-lg p-3 border border-green-200 dark:border-green-700">
                          <div class="text-xs text-gray-600 dark:text-gray-400">
                            Storage Used
                          </div>
                          <div class="text-lg font-semibold text-gray-900 dark:text-white">
                            {(accountInfo.storageUsedBytes / 1024 / 1024).toFixed(2)} MB
                          </div>
                        </div>

                        <div class="bg-white dark:bg-gray-800 rounded-lg p-3 border border-green-200 dark:border-green-700">
                          <div class="text-xs text-gray-600 dark:text-gray-400">
                            Storage Quota
                          </div>
                          <div class="text-lg font-semibold text-gray-900 dark:text-white">
                            {accountInfo.storageQuotaMb.toLocaleString()} MB
                          </div>
                        </div>
                      </div>

                      <!-- Storage Progress Bar -->
                      <div>
                        <div class="flex justify-between text-xs text-gray-600 dark:text-gray-400 mb-1">
                          <span>Storage Usage</span>
                          <span>{((accountInfo.storageUsedBytes / 1024 / 1024 / accountInfo.storageQuotaMb) * 100).toFixed(1)}%</span>
                        </div>
                        <div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2">
                          <div
                            class="bg-green-600 h-2 rounded-full"
                            style="width: {Math.min(100, (accountInfo.storageUsedBytes / 1024 / 1024 / accountInfo.storageQuotaMb) * 100)}%"
                          ></div>
                        </div>
                      </div>

                      <!-- Privacy Notice -->
                      <div class="bg-blue-50 dark:bg-blue-900/30 border border-blue-200 dark:border-blue-700 rounded p-3">
                        <div class="text-xs text-blue-900 dark:text-blue-100 space-y-1">
                          <div class="font-medium">🔒 Privacy & Security</div>
                          <div>All your notes are end-to-end encrypted. The server cannot read your content.</div>
                          <div class="mt-2">
                            <a
                              href="https://github.com/seesee/jottery"
                              target="_blank"
                              rel="noopener noreferrer"
                              class="text-blue-700 dark:text-blue-300 hover:underline font-medium"
                            >
                              💡 Want to self-host? See GitHub →
                            </a>
                          </div>
                        </div>
                      </div>

                      <!-- Delete All Notes -->
                      <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded p-3">
                        <div class="text-sm font-medium text-red-900 dark:text-red-100 mb-2">
                          ⚠️ Danger Zone
                        </div>
                        <p class="text-xs text-red-800 dark:text-red-200 mb-3">
                          Delete all your notes from the sync server. This action cannot be undone. Your local notes will NOT be deleted.
                        </p>
                        <button
                          on:click={onShowDeleteServerNotesConfirm}
                          class="px-3 py-2 bg-red-600 hover:bg-red-700 text-white text-xs font-medium rounded transition-colors"
                        >
                          Delete All Notes from Server
                        </button>
                      </div>
                    {/if}
                  </div>
    {/if}
  {/if}
</div>
