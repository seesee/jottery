<script lang="ts">
  import { createEventDispatcher, onMount } from 'svelte';
  import type { OutlinerNode } from '../../types/outliner';

  export let node: OutlinerNode;
  export let depth: number = 0;
  export let collapsed: boolean = false;
  export let collapsedNodes: Set<string> = new Set();
  export let readonly: boolean = false;
  export let isDark: boolean = false;

  const dispatch = createEventDispatcher<{
    contentChange: { id: string; content: string };
    toggleCollapse: { id: string };
    keydown: { id: string; event: KeyboardEvent; element: HTMLElement };
    focus: { id: string };
    blur: { id: string };
    dragStart: { id: string };
    dragEnd: { id: string };
    drop: { draggedId: string; targetId: string; position: 'before' | 'after' | 'child' };
  }>();

  let isDragging = false;
  let dragOverPosition: 'before' | 'after' | 'child' | null = null;

  let contentElement: HTMLElement;
  let isUserInput = false;

  // Has children means we can collapse
  $: hasChildren = node.children.length > 0;

  // Set initial content on mount
  onMount(() => {
    if (contentElement) {
      contentElement.textContent = node.content;
    }
  });

  // Update content from props only when it differs from DOM
  // and not during user input (to prevent feedback loop)
  $: if (contentElement && !isUserInput) {
    const currentContent = contentElement.textContent || '';
    if (currentContent !== node.content) {
      contentElement.textContent = node.content;
    }
  }

  function handleInput(event: Event) {
    const target = event.target as HTMLElement;
    isUserInput = true;
    dispatch('contentChange', { id: node.id, content: target.textContent || '' });
    // Reset flag after a microtask to allow the reactive update to be skipped
    queueMicrotask(() => { isUserInput = false; });
  }

  function handleKeyDown(event: KeyboardEvent) {
    dispatch('keydown', { id: node.id, event, element: contentElement });
  }

  function handleFocus() {
    dispatch('focus', { id: node.id });
  }

  function handleBlur() {
    dispatch('blur', { id: node.id });
  }

  function handleToggleCollapse() {
    if (hasChildren) {
      dispatch('toggleCollapse', { id: node.id });
    }
  }

  function handleDragStart(event: DragEvent) {
    if (readonly) return;
    isDragging = true;
    event.dataTransfer?.setData('text/plain', node.id);
    event.dataTransfer!.effectAllowed = 'move';
    dispatch('dragStart', { id: node.id });
  }

  function handleDragEnd() {
    isDragging = false;
    dragOverPosition = null;
    dispatch('dragEnd', { id: node.id });
  }

  function handleDragOver(event: DragEvent) {
    if (readonly) return;
    event.preventDefault();
    event.dataTransfer!.dropEffect = 'move';

    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const y = event.clientY - rect.top;
    const height = rect.height;

    // Determine drop position based on mouse position
    if (y < height * 0.25) {
      dragOverPosition = 'before';
    } else if (y > height * 0.75) {
      dragOverPosition = 'after';
    } else {
      dragOverPosition = 'child';
    }
  }

  function handleDragLeave() {
    dragOverPosition = null;
  }

  function handleDrop(event: DragEvent) {
    if (readonly) return;
    event.preventDefault();
    event.stopPropagation();
    const draggedId = event.dataTransfer?.getData('text/plain');
    if (draggedId && draggedId !== node.id) {
      // Calculate position directly in drop handler (dragOverPosition may be null due to dragleave timing)
      const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
      const y = event.clientY - rect.top;
      const height = rect.height;
      let position: 'before' | 'after' | 'child';
      if (y < height * 0.25) {
        position = 'before';
      } else if (y > height * 0.75) {
        position = 'after';
      } else {
        position = 'child';
      }
      dispatch('drop', { draggedId, targetId: node.id, position });
    }
    dragOverPosition = null;
  }

  // Forward events from children
  function handleChildEvent(eventName: string) {
    return (event: CustomEvent) => {
      dispatch(eventName as any, event.detail);
    };
  }

  // Export method to focus this node's content
  export function focus() {
    if (contentElement) {
      contentElement.focus();
      // Move cursor to end
      const selection = window.getSelection();
      const range = document.createRange();
      range.selectNodeContents(contentElement);
      range.collapse(false);
      selection?.removeAllRanges();
      selection?.addRange(range);
    }
  }

  // Export method to get the content element
  export function getContentElement(): HTMLElement {
    return contentElement;
  }
</script>

<div class="outliner-node" class:dragging={isDragging} data-node-id={node.id} style="--depth: {depth}">
  <div
    class="node-row"
    class:dark={isDark}
    class:drag-over-before={dragOverPosition === 'before'}
    class:drag-over-after={dragOverPosition === 'after'}
    class:drag-over-child={dragOverPosition === 'child'}
    role="listitem"
    draggable={!readonly}
    on:dragstart={handleDragStart}
    on:dragend={handleDragEnd}
    on:dragover={handleDragOver}
    on:dragleave={handleDragLeave}
    on:drop={handleDrop}
  >
    <!-- Collapse/expand bullet -->
    <button
      type="button"
      class="bullet"
      class:has-children={hasChildren}
      class:collapsed
      on:click={handleToggleCollapse}
      tabindex="-1"
      aria-label={hasChildren ? (collapsed ? 'Expand' : 'Collapse') : 'Bullet'}
      aria-expanded={hasChildren ? !collapsed : undefined}
    >
      {#if hasChildren}
        <svg class="chevron" viewBox="0 0 20 20" fill="currentColor">
          <path fill-rule="evenodd" d="M7.293 14.707a1 1 0 010-1.414L10.586 10 7.293 6.707a1 1 0 011.414-1.414l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0z" clip-rule="evenodd" />
        </svg>
      {:else}
        <span class="dot"></span>
      {/if}
    </button>

    <!-- Contenteditable content -->
    <div
      bind:this={contentElement}
      class="content"
      contenteditable={!readonly}
      role="textbox"
      tabindex="0"
      aria-multiline="false"
      on:input={handleInput}
      on:keydown={handleKeyDown}
      on:focus={handleFocus}
      on:blur={handleBlur}
    ></div>
  </div>

  <!-- Children -->
  {#if hasChildren && !collapsed}
    <div class="children">
      {#each node.children as child (child.id)}
        <svelte:self
          node={child}
          depth={depth + 1}
          collapsed={collapsedNodes.has(child.id)}
          {collapsedNodes}
          {readonly}
          {isDark}
          on:contentChange={handleChildEvent('contentChange')}
          on:toggleCollapse={handleChildEvent('toggleCollapse')}
          on:keydown={handleChildEvent('keydown')}
          on:focus={handleChildEvent('focus')}
          on:blur={handleChildEvent('blur')}
          on:dragStart={handleChildEvent('dragStart')}
          on:dragEnd={handleChildEvent('dragEnd')}
          on:drop={handleChildEvent('drop')}
        />
      {/each}
    </div>
  {/if}
</div>

<style>
  .outliner-node {
    --indent: calc(var(--depth) * 1.5rem);
  }

  .node-row {
    display: flex;
    align-items: flex-start;
    padding-left: var(--indent);
    min-height: 1.75rem;
  }

  .bullet {
    flex-shrink: 0;
    width: 1.25rem;
    height: 1.75rem;
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: none;
    padding: 0;
    cursor: pointer;
    color: #9ca3af;
    transition: color 0.15s;
  }

  .bullet:hover {
    color: #4b5563;
  }

  .dark .bullet:hover {
    color: #d1d5db;
  }

  .bullet.has-children {
    color: #6b7280;
  }

  .dark .bullet.has-children {
    color: #9ca3af;
  }

  .chevron {
    width: 1rem;
    height: 1rem;
    transition: transform 0.15s;
  }

  .bullet.collapsed .chevron {
    transform: rotate(0deg);
  }

  .bullet:not(.collapsed) .chevron {
    transform: rotate(90deg);
  }

  .dot {
    width: 0.375rem;
    height: 0.375rem;
    border-radius: 50%;
    background-color: currentColor;
  }

  .content {
    flex: 1;
    min-height: 1.75rem;
    padding: 0.125rem 0.25rem;
    line-height: 1.5rem;
    outline: none;
    border-radius: 0.25rem;
    word-break: break-word;
    white-space: pre-wrap;
  }

  .content:focus {
    background-color: rgba(59, 130, 246, 0.1);
  }

  .dark .content:focus {
    background-color: rgba(59, 130, 246, 0.2);
  }

  .content:empty::before {
    content: ' ';
    white-space: pre;
  }

  .children {
    /* Children container - no additional styling needed */
  }

  /* Drag and drop styles */
  .outliner-node.dragging {
    opacity: 0.5;
  }

  .node-row {
    cursor: grab;
    border-radius: 0.25rem;
    transition: background-color 0.15s;
  }

  .node-row:active {
    cursor: grabbing;
  }

  .node-row.drag-over-before {
    box-shadow: inset 0 2px 0 0 #3b82f6;
  }

  .node-row.drag-over-after {
    box-shadow: inset 0 -2px 0 0 #3b82f6;
  }

  .node-row.drag-over-child {
    background-color: rgba(59, 130, 246, 0.15);
  }

  .node-row.dark.drag-over-child {
    background-color: rgba(59, 130, 246, 0.25);
  }
</style>
