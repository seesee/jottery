<script lang="ts">
  // Setup configuration
  export let syncEndpoint: string;
  export let deviceName: string;

  // Registration state
  export let registrationMode: 'select' | 'newUser' | 'existingUser' | 'importCredentials';
  export let registrationStep: 'email' | 'pending' | 'device' | 'complete';
  export let userEmail: string;
  export let userPassword: string;
  export let registeringUser: boolean;
  export let registeringDevice: boolean;
  export let registeredUserId: string;
  export let userRegistrationMessage: string;

  // Import credentials
  export let importCredentialsText: string;
  export let importing: boolean;

  // Error handling
  export let syncError: string;

  // Callbacks
  export let onRegisterUser: () => void;
  export let onRegisterDevice: () => void;
  export let onResetRegistrationFlow: () => void;
  export let onImportCredentials: () => void;
</script>

<!-- Setup: Endpoint and Device Name -->
<div>
  <label for="sync-endpoint" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
    Sync Server Endpoint
  </label>
  <input
    id="sync-endpoint"
    type="url"
    bind:value={syncEndpoint}
    placeholder={typeof window !== 'undefined' ? window.location.origin : 'https://example.com'}
    class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
  />
  <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
    URL of your self-hosted Jottery sync server
  </p>
</div>

<div>
  <label for="sync-device-name" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
    Device Name
  </label>
  <input
    id="sync-device-name"
    type="text"
    bind:value={deviceName}
    placeholder="My Laptop"
    class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
  />
  <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
    A name to identify this device
  </p>
</div>

<!-- Device Sync Setup -->
{#if registrationMode === 'select'}
                <div class="space-y-3">
                  <div class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                    Choose how to set up sync:
                  </div>

                  <!-- Option 1: Import Credentials (Most Secure) -->
                  <div class="border-2 border-green-500 dark:border-green-600 rounded-lg p-4 bg-green-50 dark:bg-green-900/20">
                    <div class="flex items-start gap-3 mb-3">
                      <span class="text-2xl">🔐</span>
                      <div class="flex-1">
                        <div class="font-semibold text-green-900 dark:text-green-100 mb-1">
                          Import Sync Credentials
                        </div>
                        <div class="text-xs text-green-800 dark:text-green-200 mb-2">
                          <strong>For additional devices:</strong> Most secure method
                        </div>
                        <div class="text-xs text-green-700 dark:text-green-300 space-y-1">
                          <div>✓ Zero-knowledge encryption (server cannot decrypt your notes)</div>
                          <div>✓ Copy credentials from your first device</div>
                          <div>✓ All devices use the same encryption key</div>
                        </div>
                      </div>
                    </div>
                    <button
                      on:click={() => registrationMode = 'importCredentials'}
                      class="w-full px-4 py-2 bg-green-600 hover:bg-green-700 text-white text-sm font-medium rounded-md transition-colors"
                    >
                      📋 Import Credentials from Another Device
                    </button>
                  </div>

                  <!-- Option 2: Register with Server (First Device Only) -->
                  <div class="border-2 border-blue-500 dark:border-blue-600 rounded-lg p-4 bg-blue-50 dark:bg-blue-900/20">
                    <div class="flex items-start gap-3 mb-3">
                      <span class="text-2xl">🌐</span>
                      <div class="flex-1">
                        <div class="font-semibold text-blue-900 dark:text-blue-100 mb-1">
                          Register with a Sync Server
                        </div>
                        <div class="text-xs text-blue-800 dark:text-blue-200 mb-2">
                          <strong>For your first device only:</strong> Creates your account
                        </div>
                        <div class="text-xs text-blue-700 dark:text-blue-300 space-y-1">
                          <div>✓ Creates email/password account</div>
                          <div>✓ Enables sync for this device</div>
                          <div>✓ Requires admin approval</div>
                        </div>
                      </div>
                    </div>
                    <button
                      on:click={() => registrationMode = 'newUser'}
                      class="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-md transition-colors"
                    >
                      🚀 Register New Account
                    </button>
                  </div>

                  <!-- Info Box -->
                  <div class="bg-gray-100 dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-lg p-3">
                    <div class="text-xs text-gray-700 dark:text-gray-300 space-y-2">
                      <div>
                        <strong>🔒 Privacy:</strong> All your notes are encrypted end-to-end. The server cannot read your content.
                      </div>
                      <div>
                        <strong>💡 Self-host?</strong> Run your own server - <a href="https://github.com/seesee/jottery" target="_blank" rel="noopener noreferrer" class="text-blue-600 dark:text-blue-400 hover:underline">see GitHub</a>
                      </div>
                    </div>
                  </div>
                </div>
{:else if registrationMode === 'newUser'}
                <div class="border border-blue-200 dark:border-blue-800 rounded-lg p-4 bg-blue-50 dark:bg-blue-900/20 space-y-3">
                  <div class="flex items-center justify-between mb-2">
                    <h4 class="font-medium text-sm text-gray-900 dark:text-white">
                      Register New User Account
                    </h4>
                    <button
                      on:click={onResetRegistrationFlow}
                      class="text-xs text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white"
                    >
                      ← Back
                    </button>
                  </div>

                  {#if registrationStep === 'email'}
                    <div>
                      <label for="user-email" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                        Email Address
                      </label>
                      <input
                        id="user-email"
                        type="email"
                        bind:value={userEmail}
                        placeholder="you@example.com"
                        class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                      />
                    </div>
                    <div>
                      <label for="user-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                        Password (min 12 characters)
                      </label>
                      <input
                        id="user-password"
                        type="password"
                        bind:value={userPassword}
                        placeholder="••••••••••••"
                        class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                      />
                      <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
                        This password is for server authentication only
                      </p>
                    </div>
                    <button
                      on:click={onRegisterUser}
                      disabled={!syncEndpoint || !userEmail || !userPassword || registeringUser}
                      class="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white text-sm font-medium rounded-md transition-colors"
                    >
                      {registeringUser ? 'Registering...' : 'Register User Account'}
                    </button>
                  {:else if registrationStep === 'pending'}
                    <div class="bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded p-3">
                      <div class="flex items-start gap-2">
                        <span class="text-xl">⏳</span>
                        <div>
                          <div class="font-semibold text-sm text-yellow-900 dark:text-yellow-100">
                            Pending Admin Approval
                          </div>
                          <p class="text-xs text-yellow-800 dark:text-yellow-200 mt-1">
                            {userRegistrationMessage}
                          </p>
                          <p class="text-xs text-yellow-700 dark:text-yellow-300 mt-2">
                            Contact your administrator to approve your account. Once approved, click below to complete setup.
                          </p>
                        </div>
                      </div>
                    </div>
                    <button
                      on:click={onRegisterDevice}
                      disabled={registeringDevice}
                      class="w-full px-4 py-2 bg-green-600 hover:bg-green-700 disabled:bg-green-400 text-white text-sm font-medium rounded-md transition-colors"
                    >
                      {registeringDevice ? 'Completing Setup...' : 'I\'ve Been Approved - Complete Setup'}
                    </button>
                  {:else if registrationStep === 'device'}
                    <div class="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded p-3 mb-3">
                      <div class="text-xs text-green-800 dark:text-green-200">
                        ⏳ Registering device and syncing...
                      </div>
                    </div>
                  {:else if registrationStep === 'complete'}
                    <div class="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded p-3">
                      <div class="flex items-start gap-2">
                        <span class="text-xl">✅</span>
                        <div>
                          <div class="font-semibold text-sm text-green-900 dark:text-green-100">
                            Sync Enabled Successfully!
                          </div>
                          <p class="text-xs text-green-800 dark:text-green-200 mt-2">
                            Your device is now registered and <strong>all your existing notes have been synced to the server</strong>. Any future changes will sync automatically.
                          </p>
                          <p class="text-xs text-green-700 dark:text-green-300 mt-2 font-medium">
                            📋 To add another device: Click "Export Sync Credentials" below and import them on your other device.
                          </p>
                        </div>
                      </div>
                    </div>
                    <button
                      on:click={onResetRegistrationFlow}
                      class="w-full px-4 py-2 bg-gray-600 hover:bg-gray-700 text-white text-sm font-medium rounded-md transition-colors mt-2"
                    >
                      Done
                    </button>
                  {/if}
                </div>
{:else if registrationMode === 'existingUser'}
                <div class="border border-gray-300 dark:border-gray-600 rounded-lg p-4 bg-gray-50 dark:bg-gray-800 space-y-3">
                  <div class="flex items-center justify-between mb-2">
                    <h4 class="font-medium text-sm text-gray-900 dark:text-white">
                      Link Account (Optional)
                    </h4>
                    <button
                      on:click={onResetRegistrationFlow}
                      class="text-xs text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white"
                    >
                      ← Back
                    </button>
                  </div>

                  <div class="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-700 rounded p-3 mb-3">
                    <p class="text-xs text-blue-800 dark:text-blue-200">
                      ℹ️ This feature is for future use (admin dashboard access, subscription management). It does <strong>not</strong> enable sync on this device.
                    </p>
                  </div>

                  <div>
                    <label for="existing-email" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                      Email Address
                    </label>
                    <input
                      id="existing-email"
                      type="email"
                      bind:value={userEmail}
                      placeholder="you@example.com"
                      class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                    />
                  </div>
                  <div>
                    <label for="existing-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                      Password
                    </label>
                    <input
                      id="existing-password"
                      type="password"
                      bind:value={userPassword}
                      placeholder="••••••••••••"
                      class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                    />
                  </div>
                  <button
                    disabled
                    class="w-full px-4 py-2 bg-gray-400 text-white text-sm font-medium rounded-md cursor-not-allowed"
                  >
                    Coming Soon
                  </button>
                  <p class="text-xs text-gray-500 dark:text-gray-400 text-center">
                    Account linking will be enabled in a future update
                  </p>
                </div>
{:else if registrationMode === 'importCredentials'}
                <div class="border border-green-200 dark:border-green-800 rounded-lg p-4 bg-green-50 dark:bg-green-900/20 space-y-3">
                  <div class="flex items-center justify-between mb-2">
                    <h4 class="font-medium text-sm text-gray-900 dark:text-white">
                      Import Credentials
                    </h4>
                    <button
                      on:click={onResetRegistrationFlow}
                      class="text-xs text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white"
                    >
                      ← Back
                    </button>
                  </div>

                  <p class="text-xs text-gray-600 dark:text-gray-400">
                    Paste the credentials from your first device. The app will lock and you'll need to unlock with your password.
                  </p>

                  <div class="bg-orange-100 dark:bg-orange-900/30 border border-orange-300 dark:border-orange-700 rounded p-2">
                    <p class="text-xs text-orange-800 dark:text-orange-200 font-medium">
                      ⚠️ IMPORTANT: You must use the SAME password on all devices!
                    </p>
                    <p class="text-xs text-orange-700 dark:text-orange-300 mt-1">
                      If you use a different password, notes will not decrypt.
                    </p>
                  </div>

                  <textarea
                    bind:value={importCredentialsText}
                    placeholder="Paste base64 credentials here..."
                    rows="4"
                    class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500 font-mono text-xs"
                  ></textarea>
                  <button
                    on:click={onImportCredentials}
                    disabled={!importCredentialsText.trim() || importing}
                    class="w-full px-4 py-2 bg-green-600 hover:bg-green-700 disabled:bg-green-400 text-white text-sm font-medium rounded-md transition-colors"
                  >
                    {importing ? 'Importing...' : '📥 Import and Lock'}
                  </button>
                </div>
              {/if}

<!-- Error Display -->
{#if syncError}
                <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-3">
                  <div class="flex items-start gap-2">
                    <span class="text-xl">⚠️</span>
                    <div>
                      <div class="font-semibold text-sm text-red-900 dark:text-red-100">
                        Error
                      </div>
                      <p class="text-xs text-red-800 dark:text-red-200 mt-1">
                        {syncError}
                      </p>
                    </div>
                  </div>
                </div>
              {/if}
