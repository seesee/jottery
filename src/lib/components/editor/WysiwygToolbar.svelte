<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { WysiwygEditorRef } from '../../types';

  // Editor reference (WysiwygEditor component)
  export let editor: WysiwygEditorRef | null = null;
  export let disabled: boolean = false;

  // Toolbar state
  let isBold = false;
  let isItalic = false;
  let isStrike = false;
  let isCode = false;
  let isBulletList = false;
  let isOrderedList = false;
  let isTaskList = false;
  let isBlockquote = false;
  let isCodeBlock = false;
  let isHeading1 = false;
  let isHeading2 = false;
  let isHeading3 = false;

  // Update toolbar state when editor changes
  $: if (editor) {
    const tiptapEditor = editor.getEditor?.();
    if (tiptapEditor) {
      isBold = tiptapEditor.isActive('bold');
      isItalic = tiptapEditor.isActive('italic');
      isStrike = tiptapEditor.isActive('strike');
      isCode = tiptapEditor.isActive('code');
      isBulletList = tiptapEditor.isActive('bulletList');
      isOrderedList = tiptapEditor.isActive('orderedList');
      isTaskList = tiptapEditor.isActive('taskList');
      isBlockquote = tiptapEditor.isActive('blockquote');
      isCodeBlock = tiptapEditor.isActive('codeBlock');
      isHeading1 = tiptapEditor.isActive('heading', { level: 1 });
      isHeading2 = tiptapEditor.isActive('heading', { level: 2 });
      isHeading3 = tiptapEditor.isActive('heading', { level: 3 });
    }
  }

  function handleBold() {
    editor?.toggleBold();
  }

  function handleItalic() {
    editor?.toggleItalic();
  }

  function handleStrike() {
    editor?.toggleStrike();
  }

  function handleCode() {
    editor?.toggleCode();
  }

  function handleHeading(level: 1 | 2 | 3) {
    editor?.toggleHeading(level);
  }

  function handleBulletList() {
    editor?.toggleBulletList();
  }

  function handleOrderedList() {
    editor?.toggleOrderedList();
  }

  function handleTaskList() {
    editor?.toggleTaskList();
  }

  function handleBlockquote() {
    editor?.toggleBlockquote();
  }

  function handleCodeBlock() {
    editor?.toggleCodeBlock();
  }

  function handleLink() {
    const url = prompt($_('editor.wysiwyg.enterUrl'));
    if (url !== null) {
      editor?.setLink(url);
    }
  }

  function handleHorizontalRule() {
    editor?.insertHorizontalRule();
  }

  function handleTable() {
    editor?.insertTable(3, 3);
  }

  $: buttonClass = disabled
    ? "p-1.5 rounded text-gray-400 dark:text-gray-600 cursor-not-allowed"
    : "p-1.5 rounded hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors text-gray-700 dark:text-gray-300";
  const activeClass = "bg-blue-100 dark:bg-blue-900/50 text-blue-700 dark:text-blue-300";
</script>

<div class="flex flex-wrap items-center gap-0.5 p-1 border-b border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50">
  <!-- Text formatting -->
  <div class="flex items-center gap-0.5 pr-2 border-r border-gray-300 dark:border-gray-600">
    <button
      type="button"
      on:click={handleBold}
      {disabled}
      class="{buttonClass} {isBold ? activeClass : ''}"
      title={$_('editor.wysiwyg.bold')}
      aria-label={$_('editor.wysiwyg.bold')}
      aria-pressed={isBold}
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 4h8a4 4 0 014 4 4 4 0 01-4 4H6z M6 12h9a4 4 0 014 4 4 4 0 01-4 4H6z" />
      </svg>
    </button>
    <button
      type="button"
      on:click={handleItalic}
      {disabled}
      class="{buttonClass} {isItalic ? activeClass : ''}"
      title={$_('editor.wysiwyg.italic')}
      aria-label={$_('editor.wysiwyg.italic')}
      aria-pressed={isItalic}
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 4h4m2 0l-6 16m-2 0h4" />
      </svg>
    </button>
    <button
      type="button"
      on:click={handleStrike}
      {disabled}
      class="{buttonClass} {isStrike ? activeClass : ''}"
      title={$_('editor.wysiwyg.strikethrough')}
      aria-label={$_('editor.wysiwyg.strikethrough')}
      aria-pressed={isStrike}
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 12H7m10 0a4 4 0 01-4 4H7m10-4a4 4 0 00-4-4H7" />
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 12h16" />
      </svg>
    </button>
    <button
      type="button"
      on:click={handleCode}
      {disabled}
      class="{buttonClass} {isCode ? activeClass : ''}"
      title={$_('editor.wysiwyg.inlineCode')}
      aria-label={$_('editor.wysiwyg.inlineCode')}
      aria-pressed={isCode}
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4" />
      </svg>
    </button>
  </div>

  <!-- Headings -->
  <div class="flex items-center gap-0.5 px-2 border-r border-gray-300 dark:border-gray-600">
    <button
      type="button"
      on:click={() => handleHeading(1)}
      {disabled}
      class="{buttonClass} {isHeading1 ? activeClass : ''} text-xs font-bold"
      title={$_('editor.wysiwyg.heading1')}
      aria-pressed={isHeading1}
    >
      H1
    </button>
    <button
      type="button"
      on:click={() => handleHeading(2)}
      {disabled}
      class="{buttonClass} {isHeading2 ? activeClass : ''} text-xs font-bold"
      title={$_('editor.wysiwyg.heading2')}
      aria-pressed={isHeading2}
    >
      H2
    </button>
    <button
      type="button"
      on:click={() => handleHeading(3)}
      {disabled}
      class="{buttonClass} {isHeading3 ? activeClass : ''} text-xs font-bold"
      title={$_('editor.wysiwyg.heading3')}
      aria-pressed={isHeading3}
    >
      H3
    </button>
  </div>

  <!-- Lists -->
  <div class="flex items-center gap-0.5 px-2 border-r border-gray-300 dark:border-gray-600">
    <button
      type="button"
      on:click={handleBulletList}
      {disabled}
      class="{buttonClass} {isBulletList ? activeClass : ''}"
      title={$_('editor.wysiwyg.bulletList')}
      aria-label={$_('editor.wysiwyg.bulletList')}
      aria-pressed={isBulletList}
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
        <circle cx="2" cy="6" r="1" fill="currentColor" />
        <circle cx="2" cy="12" r="1" fill="currentColor" />
        <circle cx="2" cy="18" r="1" fill="currentColor" />
      </svg>
    </button>
    <button
      type="button"
      on:click={handleOrderedList}
      {disabled}
      class="{buttonClass} {isOrderedList ? activeClass : ''}"
      title={$_('editor.wysiwyg.orderedList')}
      aria-label={$_('editor.wysiwyg.orderedList')}
      aria-pressed={isOrderedList}
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 6h13M7 12h13M7 18h13" />
        <text x="1" y="8" font-size="6" fill="currentColor">1</text>
        <text x="1" y="14" font-size="6" fill="currentColor">2</text>
        <text x="1" y="20" font-size="6" fill="currentColor">3</text>
      </svg>
    </button>
    <button
      type="button"
      on:click={handleTaskList}
      {disabled}
      class="{buttonClass} {isTaskList ? activeClass : ''}"
      title={$_('editor.wysiwyg.taskList')}
      aria-label={$_('editor.wysiwyg.taskList')}
      aria-pressed={isTaskList}
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
        <rect x="3" y="5" width="4" height="4" rx="0.5" stroke-width="2" />
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 7h10" />
        <rect x="3" y="13" width="4" height="4" rx="0.5" stroke-width="2" />
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4.5 15l1 1 2-2" />
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 15h10" />
      </svg>
    </button>
  </div>

  <!-- Blocks -->
  <div class="flex items-center gap-0.5 px-2 border-r border-gray-300 dark:border-gray-600">
    <button
      type="button"
      on:click={handleBlockquote}
      {disabled}
      class="{buttonClass} {isBlockquote ? activeClass : ''}"
      title={$_('editor.wysiwyg.blockquote')}
      aria-label={$_('editor.wysiwyg.blockquote')}
      aria-pressed={isBlockquote}
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 10h.01M12 10h.01M16 10h.01M9 16H5a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v8a2 2 0 01-2 2h-5l-5 5v-5z" />
      </svg>
    </button>
    <button
      type="button"
      on:click={handleCodeBlock}
      {disabled}
      class="{buttonClass} {isCodeBlock ? activeClass : ''}"
      title={$_('editor.wysiwyg.codeBlock')}
      aria-label={$_('editor.wysiwyg.codeBlock')}
      aria-pressed={isCodeBlock}
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
        <rect x="3" y="3" width="18" height="18" rx="2" stroke-width="2" />
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 10l-2 2 2 2M16 10l2 2-2 2M13 8l-2 8" />
      </svg>
    </button>
    <button
      type="button"
      on:click={handleHorizontalRule}
      {disabled}
      class={buttonClass}
      title={$_('editor.wysiwyg.horizontalRule')}
      aria-label={$_('editor.wysiwyg.horizontalRule')}
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 12h16" />
      </svg>
    </button>
  </div>

  <!-- Insert -->
  <div class="flex items-center gap-0.5 pl-2">
    <button
      type="button"
      on:click={handleLink}
      {disabled}
      class={buttonClass}
      title={$_('editor.wysiwyg.link')}
      aria-label={$_('editor.wysiwyg.link')}
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" />
      </svg>
    </button>
    <button
      type="button"
      on:click={handleTable}
      {disabled}
      class={buttonClass}
      title={$_('editor.wysiwyg.table')}
      aria-label={$_('editor.wysiwyg.table')}
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
        <rect x="3" y="3" width="18" height="18" rx="2" stroke-width="2" />
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 9h18M3 15h18M9 3v18M15 3v18" />
      </svg>
    </button>
  </div>
</div>
