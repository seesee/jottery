<script lang="ts">
  import { _ } from 'svelte-i18n';

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
  export let _registeredUserId: string;
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
    {$_('settings.syncSetup.endpoint.label')}
  </label>
  <input
    id="sync-endpoint"
    type="url"
    bind:value={syncEndpoint}
    placeholder={typeof window !== 'undefined' ? window.location.origin : 'https://example.com'}
    class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
  />
  <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
    {$_('settings.syncSetup.endpoint.help')}
  </p>
</div>

<div>
  <label for="sync-device-name" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
    {$_('settings.syncSetup.deviceName.label')}
  </label>
  <input
    id="sync-device-name"
    type="text"
    bind:value={deviceName}
    placeholder={$_('settings.syncSetup.deviceName.placeholder')}
    class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
  />
  <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
    {$_('settings.syncSetup.deviceName.help')}
  </p>
</div>

<!-- Device Sync Setup -->
{#if registrationMode === 'select'}
                <div class="space-y-3">
                  <div class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                    {$_('settings.syncSetup.chooseMethod')}
                  </div>

                  <!-- Option 1: Import Credentials (Most Secure) -->
                  <div class="border-2 border-green-500 dark:border-green-600 rounded-lg p-4 bg-green-50 dark:bg-green-900/20">
                    <div class="flex items-start gap-3 mb-3">
                      <span class="text-2xl">🔐</span>
                      <div class="flex-1">
                        <div class="font-semibold text-green-900 dark:text-green-100 mb-1">
                          {$_('settings.syncSetup.importCredentials.title')}
                        </div>
                        <div class="text-xs text-green-800 dark:text-green-200 mb-2">
                          {@html $_('settings.syncSetup.importCredentials.subtitle')}
                        </div>
                        <div class="text-xs text-green-700 dark:text-green-300 space-y-1">
                          <div>{$_('settings.syncSetup.importCredentials.benefit1')}</div>
                          <div>{$_('settings.syncSetup.importCredentials.benefit2')}</div>
                          <div>{$_('settings.syncSetup.importCredentials.benefit3')}</div>
                        </div>
                      </div>
                    </div>
                    <button
                      on:click={() => registrationMode = 'importCredentials'}
                      class="w-full px-4 py-2 bg-green-600 hover:bg-green-700 text-white text-sm font-medium rounded-md transition-colors"
                    >
                      {$_('settings.syncSetup.importCredentials.button')}
                    </button>
                  </div>

                  <!-- Option 2: Register with Server (First Device Only) -->
                  <div class="border-2 border-blue-500 dark:border-blue-600 rounded-lg p-4 bg-blue-50 dark:bg-blue-900/20">
                    <div class="flex items-start gap-3 mb-3">
                      <span class="text-2xl">🌐</span>
                      <div class="flex-1">
                        <div class="font-semibold text-blue-900 dark:text-blue-100 mb-1">
                          {$_('settings.syncSetup.registerServer.title')}
                        </div>
                        <div class="text-xs text-blue-800 dark:text-blue-200 mb-2">
                          {@html $_('settings.syncSetup.registerServer.subtitle')}
                        </div>
                        <div class="text-xs text-blue-700 dark:text-blue-300 space-y-1">
                          <div>{$_('settings.syncSetup.registerServer.benefit1')}</div>
                          <div>{$_('settings.syncSetup.registerServer.benefit2')}</div>
                          <div>{$_('settings.syncSetup.registerServer.benefit3')}</div>
                        </div>
                      </div>
                    </div>
                    <button
                      on:click={() => registrationMode = 'newUser'}
                      class="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-md transition-colors"
                    >
                      {$_('settings.syncSetup.registerServer.button')}
                    </button>
                  </div>

                  <!-- Info Box -->
                  <div class="bg-gray-100 dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-lg p-3">
                    <div class="text-xs text-gray-700 dark:text-gray-300 space-y-2">
                      <div>
                        {@html $_('settings.syncSetup.privacyNote')}
                      </div>
                      <div>
                        {@html $_('settings.syncSetup.selfHostNote')}
                      </div>
                    </div>
                  </div>
                </div>
{:else if registrationMode === 'newUser'}
                <div class="border border-blue-200 dark:border-blue-800 rounded-lg p-4 bg-blue-50 dark:bg-blue-900/20 space-y-3">
                  <div class="flex items-center justify-between mb-2">
                    <h4 class="font-medium text-sm text-gray-900 dark:text-white">
                      {$_('settings.syncSetup.registration.title')}
                    </h4>
                    <button
                      on:click={onResetRegistrationFlow}
                      class="text-xs text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white"
                    >
                      {$_('settings.syncSetup.back')}
                    </button>
                  </div>

                  {#if registrationStep === 'email'}
                    <div>
                      <label for="user-email" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                        {$_('settings.syncSetup.registration.emailLabel')}
                      </label>
                      <input
                        id="user-email"
                        type="email"
                        bind:value={userEmail}
                        placeholder={$_('settings.syncSetup.registration.emailPlaceholder')}
                        class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                      />
                    </div>
                    <div>
                      <label for="user-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                        {$_('settings.syncSetup.registration.passwordLabel')}
                      </label>
                      <input
                        id="user-password"
                        type="password"
                        bind:value={userPassword}
                        placeholder={$_('settings.syncSetup.registration.passwordPlaceholder')}
                        class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                      />
                      <div class="mt-2 bg-amber-50 dark:bg-amber-900/20 border border-amber-300 dark:border-amber-700 rounded p-2">
                        <p class="text-xs text-amber-800 dark:text-amber-200 font-medium flex items-start gap-1">
                          <span>💡</span>
                          <span>{@html $_('settings.syncSetup.registration.passwordWarningTitle')}</span>
                        </p>
                        <p class="text-xs text-amber-700 dark:text-amber-300 mt-1 ml-5">
                          {$_('settings.syncSetup.registration.passwordWarningText')}
                        </p>
                      </div>
                    </div>
                    <button
                      on:click={onRegisterUser}
                      disabled={!syncEndpoint || !userEmail || !userPassword || registeringUser}
                      class="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white text-sm font-medium rounded-md transition-colors"
                    >
                      {registeringUser ? $_('settings.syncSetup.registration.registering') : $_('settings.syncSetup.registration.registerButton')}
                    </button>
                  {:else if registrationStep === 'pending'}
                    <div class="bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded p-3">
                      <div class="flex items-start gap-2">
                        <span class="text-xl">⏳</span>
                        <div>
                          <div class="font-semibold text-sm text-yellow-900 dark:text-yellow-100">
                            {$_('settings.syncSetup.registration.pendingTitle')}
                          </div>
                          <p class="text-xs text-yellow-800 dark:text-yellow-200 mt-1">
                            {userRegistrationMessage}
                          </p>
                          <p class="text-xs text-yellow-700 dark:text-yellow-300 mt-2">
                            {$_('settings.syncSetup.registration.pendingText')}
                          </p>
                        </div>
                      </div>
                    </div>
                    <button
                      on:click={onRegisterDevice}
                      disabled={registeringDevice}
                      class="w-full px-4 py-2 bg-green-600 hover:bg-green-700 disabled:bg-green-400 text-white text-sm font-medium rounded-md transition-colors"
                    >
                      {registeringDevice ? $_('settings.syncSetup.registration.completingSetup') : $_('settings.syncSetup.registration.approvedButton')}
                    </button>
                  {:else if registrationStep === 'device'}
                    <div class="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded p-3 mb-3">
                      <div class="text-xs text-green-800 dark:text-green-200">
                        {$_('settings.syncSetup.registration.deviceRegistering')}
                      </div>
                    </div>
                  {:else if registrationStep === 'complete'}
                    <div class="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded p-3">
                      <div class="flex items-start gap-2">
                        <span class="text-xl">✅</span>
                        <div>
                          <div class="font-semibold text-sm text-green-900 dark:text-green-100">
                            {$_('settings.syncSetup.registration.successTitle')}
                          </div>
                          <p class="text-xs text-green-800 dark:text-green-200 mt-2">
                            {@html $_('settings.syncSetup.registration.successText')}
                          </p>
                          <p class="text-xs text-green-700 dark:text-green-300 mt-2 font-medium">
                            {$_('settings.syncSetup.registration.successNextSteps')}
                          </p>
                        </div>
                      </div>
                    </div>
                    <button
                      on:click={onResetRegistrationFlow}
                      class="w-full px-4 py-2 bg-gray-600 hover:bg-gray-700 text-white text-sm font-medium rounded-md transition-colors mt-2"
                    >
                      {$_('settings.syncSetup.registration.done')}
                    </button>
                  {/if}
                </div>
{:else if registrationMode === 'existingUser'}
                <div class="border border-gray-300 dark:border-gray-600 rounded-lg p-4 bg-gray-50 dark:bg-gray-800 space-y-3">
                  <div class="flex items-center justify-between mb-2">
                    <h4 class="font-medium text-sm text-gray-900 dark:text-white">
                      {$_('settings.syncSetup.linkAccount.title')}
                    </h4>
                    <button
                      on:click={onResetRegistrationFlow}
                      class="text-xs text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white"
                    >
                      {$_('settings.syncSetup.back')}
                    </button>
                  </div>

                  <div class="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-700 rounded p-3 mb-3">
                    <p class="text-xs text-blue-800 dark:text-blue-200">
                      {@html $_('settings.syncSetup.linkAccount.infoText')}
                    </p>
                  </div>

                  <div>
                    <label for="existing-email" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                      {$_('settings.syncSetup.linkAccount.emailLabel')}
                    </label>
                    <input
                      id="existing-email"
                      type="email"
                      bind:value={userEmail}
                      placeholder={$_('settings.syncSetup.registration.emailPlaceholder')}
                      class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                    />
                  </div>
                  <div>
                    <label for="existing-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                      {$_('settings.syncSetup.linkAccount.passwordLabel')}
                    </label>
                    <input
                      id="existing-password"
                      type="password"
                      bind:value={userPassword}
                      placeholder={$_('settings.syncSetup.registration.passwordPlaceholder')}
                      class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                    />
                  </div>
                  <button
                    disabled
                    class="w-full px-4 py-2 bg-gray-400 text-white text-sm font-medium rounded-md cursor-not-allowed"
                  >
                    {$_('settings.syncSetup.linkAccount.comingSoon')}
                  </button>
                  <p class="text-xs text-gray-500 dark:text-gray-400 text-center">
                    {$_('settings.syncSetup.linkAccount.futureUpdate')}
                  </p>
                </div>
{:else if registrationMode === 'importCredentials'}
                <div class="border border-green-200 dark:border-green-800 rounded-lg p-4 bg-green-50 dark:bg-green-900/20 space-y-3">
                  <div class="flex items-center justify-between mb-2">
                    <h4 class="font-medium text-sm text-gray-900 dark:text-white">
                      {$_('settings.syncSetup.import.title')}
                    </h4>
                    <button
                      on:click={onResetRegistrationFlow}
                      class="text-xs text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white"
                    >
                      {$_('settings.syncSetup.back')}
                    </button>
                  </div>

                  <p class="text-xs text-gray-600 dark:text-gray-400">
                    {$_('settings.syncSetup.import.instructions')}
                  </p>

                  <div class="bg-orange-100 dark:bg-orange-900/30 border border-orange-300 dark:border-orange-700 rounded p-2">
                    <p class="text-xs text-orange-800 dark:text-orange-200 font-medium">
                      {$_('settings.syncSetup.import.warningTitle')}
                    </p>
                    <p class="text-xs text-orange-700 dark:text-orange-300 mt-1">
                      {$_('settings.syncSetup.import.warningText')}
                    </p>
                  </div>

                  <textarea
                    bind:value={importCredentialsText}
                    placeholder={$_('settings.syncSetup.import.placeholder')}
                    rows="4"
                    class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500 font-mono text-xs"
                  ></textarea>
                  <button
                    on:click={onImportCredentials}
                    disabled={!importCredentialsText.trim() || importing}
                    class="w-full px-4 py-2 bg-green-600 hover:bg-green-700 disabled:bg-green-400 text-white text-sm font-medium rounded-md transition-colors"
                  >
                    {importing ? $_('settings.syncSetup.import.importing') : $_('settings.syncSetup.import.button')}
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
                        {$_('settings.syncSetup.error')}
                      </div>
                      <p class="text-xs text-red-800 dark:text-red-200 mt-1">
                        {syncError}
                      </p>
                    </div>
                  </div>
                </div>
              {/if}
