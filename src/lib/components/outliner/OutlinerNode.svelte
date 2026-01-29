<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { OutlinerNode } from '../../types/outliner';

  export let node: OutlinerNode;
  export let depth: number = 0;
  export let collapsed: boolean = false;
  export let readonly: boolean = false;
  export let isDark: boolean = false;

  const dispatch = createEventDispatcher<{
    contentChange: { id: string; content: string };
    toggleCollapse: { id: string };
    keydown: { id: string; event: KeyboardEvent; element: HTMLElement };
    focus: { id: string };
    blur: { id: string };
  }>();

  let contentElement: HTMLElement;

  // Has children means we can collapse
  $: hasChildren = node.children.length > 0;

  function handleInput(event: Event) {
    const target = event.target as HTMLElement;
    dispatch('contentChange', { id: node.id, content: target.textContent || '' });
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

<div class="outliner-node" data-node-id={node.id} style="--depth: {depth}">
  <div class="node-row" class:dark={isDark}>
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
      aria-multiline="false"
      on:input={handleInput}
      on:keydown={handleKeyDown}
      on:focus={handleFocus}
      on:blur={handleBlur}
    >{node.content}</div>
  </div>

  <!-- Children -->
  {#if hasChildren && !collapsed}
    <div class="children">
      {#each node.children as child (child.id)}
        <svelte:self
          node={child}
          depth={depth + 1}
          collapsed={false}
          {readonly}
          {isDark}
          on:contentChange={handleChildEvent('contentChange')}
          on:toggleCollapse={handleChildEvent('toggleCollapse')}
          on:keydown={handleChildEvent('keydown')}
          on:focus={handleChildEvent('focus')}
          on:blur={handleChildEvent('blur')}
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
</style>
