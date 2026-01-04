<script lang="ts">
  import { _ } from 'svelte-i18n';
  import CodeEditor from '../CodeEditor.svelte';
  import TagInput from '../TagInput.svelte';

  // State props
  export let showPreview: boolean;
  export let canPreview: boolean;
  export let content: string;
  export let tags: string[];
  export let language: string;
  export let wordWrap: boolean;
  export let isDark: boolean;
  export let availableTags: string[];

  // Editor ref (for bind:this)
  export let codeEditor: any = null;

  // Callbacks
  export let onContentChange: (newContent: string) => void;
  export let onTagsChange: (newTags: string[]) => void;
  export let onTagClick: ((tag: string) => void) | undefined = undefined;

  // Preview HTML
  export let previewHtml: string;

  // Handler wrappers
  function handleContentChange(newValue: string) {
    content = newValue;
    onContentChange(newValue);
  }

  function handleTagsChange(newTags: string[]) {
    tags = newTags;
    onTagsChange(newTags);
  }
</script>

<!-- Tags Input -->
<div class="border-b border-gray-200 dark:border-gray-700 p-2">
  <TagInput
    bind:tags
    onChange={handleTagsChange}
    {availableTags}
    placeholder={$_('editor.addTags')}
    {onTagClick}
  />
</div>

<!-- Content Editor with CodeMirror OR Preview (swap, not side-by-side) -->
<div class="flex-1 overflow-hidden bg-white dark:bg-gray-900">
  <!-- Editor - Only shown when NOT in preview mode -->
  {#if !showPreview}
    <div class="h-full overflow-hidden">
      <CodeEditor
        bind:this={codeEditor}
        value={content}
        onChange={handleContentChange}
        {language}
        {wordWrap}
        {isDark}
      />
    </div>
  {/if}

  <!-- Preview Panel - Only shown when IN preview mode -->
  {#if showPreview && canPreview}
    <div class="h-full overflow-auto p-8 bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100">
      <div class="prose dark:prose-invert max-w-none">
        {@html previewHtml}
      </div>
    </div>
  {/if}
</div>
