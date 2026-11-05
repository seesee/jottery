<script lang="ts">
  import { isUserPortal } from './lib/appMode';
  import { _, locale } from 'svelte-i18n';
  import AdminApp from './AdminApp.svelte';
  import UserApp from './UserApp.svelte';

  // index.html only knows the default language, and serves both apps
  $effect(() => {
    document.documentElement.lang = $locale ?? 'en-GB';
    document.title = isUserPortal ? `Jottery: ${$_('userPortal.title')}` : $_('app.name');
  });
</script>

{#if isUserPortal}
  <UserApp />
{:else}
  <AdminApp />
{/if}
