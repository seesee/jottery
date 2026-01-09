<script lang="ts">
  import { isUserPortal } from './lib/appMode';
  import AdminApp from './AdminApp.svelte';
  import UserApp from './UserApp.svelte';
</script>

{#if isUserPortal}
  <UserApp />
{:else}
  <AdminApp />
{/if}
