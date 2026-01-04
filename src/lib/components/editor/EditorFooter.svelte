<script lang="ts">
  import { _ } from 'svelte-i18n';

  export let createdAtFormatted: string | null;
  export let modifiedAtFormatted: string | null;
</script>

<!-- Metadata Footer -->
<div class="border-t border-gray-200 dark:border-gray-700 p-2 text-xs text-gray-500 dark:text-gray-400">
  <div class="flex justify-between">
    <span>{$_('note.created')}: {createdAtFormatted || ''}</span>
    <span>{$_('note.modified')}: {modifiedAtFormatted || ''}</span>
  </div>
</div>
