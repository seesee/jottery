<script lang="ts">
  import { onMount } from 'svelte';
  import { isLocked, notes, settings } from './lib/stores/appStore';
  import { initDB, noteService, settingsRepository, isLocked as checkLocked } from './lib/services';
  import UnlockScreen from './lib/components/UnlockScreen.svelte';
  import Header from './lib/components/Header.svelte';
  import NoteList from './lib/components/NoteList.svelte';
  import EditorPane from './lib/components/EditorPane.svelte';

  let initialized = false;

  onMount(async () => {
    try {
      // Initialize database
      await initDB();

      // Load settings
      const userSettings = await settingsRepository.get();
      settings.set(userSettings);

      // Check lock status
      isLocked.set(checkLocked());

      initialized = true;
    } catch (error) {
      console.error('Failed to initialize app:', error);
    }
  });

  // Watch lock status and load notes when unlocked
  $: if (!$isLocked && initialized) {
    loadNotes();
  }

  async function loadNotes() {
    try {
      const allNotes = await noteService.getAllNotes($settings.sortOrder);
      notes.set(allNotes);
    } catch (error) {
      console.error('Failed to load notes:', error);
    }
  }

  // Reload notes when lock status changes
  $: if (!$isLocked) {
    loadNotes();
  } else if ($isLocked) {
    notes.set([]);
  }
</script>

{#if !initialized}
  <div class="h-screen w-screen flex items-center justify-center bg-gray-50 dark:bg-gray-900">
    <div class="text-center">
      <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600 mx-auto mb-4"></div>
      <p class="text-gray-600 dark:text-gray-400">Loading...</p>
    </div>
  </div>
{:else if $isLocked}
  <UnlockScreen />
{:else}
  <div class="h-screen w-screen bg-gray-50 dark:bg-gray-900 text-gray-900 dark:text-gray-100">
    <div class="flex h-full flex-col">
      <Header />

      <main class="flex-1 overflow-hidden flex">
        <!-- Note List Sidebar -->
        <div class="w-80 border-r border-gray-200 dark:border-gray-700">
          <NoteList />
        </div>

        <!-- Editor -->
        <div class="flex-1">
          <EditorPane />
        </div>
      </main>
    </div>
  </div>
{/if}
