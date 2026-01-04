<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { Readable } from 'svelte/store';

  export let createdAtFormatted: Readable<string> | null;
  export let modifiedAtFormatted: Readable<string> | null;
</script>

<!-- Metadata Footer -->
<div class="border-t border-gray-200 dark:border-gray-700 p-2 text-xs text-gray-500 dark:text-gray-400">
  <div class="flex justify-between">
    <span>{$_('note.created')}: {createdAtFormatted ? $createdAtFormatted : ''}</span>
    <span>{$_('note.modified')}: {modifiedAtFormatted ? $modifiedAtFormatted : ''}</span>
  </div>
</div>
