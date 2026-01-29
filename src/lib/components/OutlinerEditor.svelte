<script context="module" lang="ts">
  // Module-level state for drag tracking (shared across instances, survives reactivity)
  let currentDraggedNodeId: string | null = null;
</script>

<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { _ } from 'svelte-i18n';
  import type { OutlinerNode } from '../types/outliner';
  import {
    parseOutliner,
    serialiseOutliner,
    findNodeById,
    findParentNode,
    getPreviousNode,
    createNode,
    flattenNodes,
  } from '../utils/outlinerParser';
  import OutlinerNodeComponent from './outliner/OutlinerNode.svelte';
  import { settings } from '../stores/appStore';
  import { getFontSize } from '../utils/fontSize';

  export let value: string = '';
  export let onChange: (value: string) => void = () => {};
  export let readonly: boolean = false;
  export let isDark: boolean = false;

  // Internal state
  let nodes: OutlinerNode[] = [];
  let collapsedNodes: Set<string> = new Set();
  let containerElement: HTMLDivElement;
  let showToolbar = true;

  // Font size from settings
  $: fontSize = getFontSize($settings.fontSize);

  // Build collapsed set from nodes tree
  function buildCollapsedSet(nodeList: OutlinerNode[]): Set<string> {
    const set = new Set<string>();
    function traverse(n: OutlinerNode[]) {
      for (const node of n) {
        if (node.collapsed) set.add(node.id);
        traverse(node.children);
      }
    }
    traverse(nodeList);
    return set;
  }

  // Parse value on mount and when value changes externally
  onMount(() => {
    nodes = parseOutliner(value);
    // If empty, create a default node
    if (nodes.length === 0) {
      nodes = [createNode()];
      emitChange();
    } else {
      collapsedNodes = buildCollapsedSet(nodes);
    }
  });

  // Handle external value changes
  $: {
    const currentSerialized = serialiseOutliner(nodes);
    if (value !== currentSerialized && value !== undefined) {
      const newNodes = parseOutliner(value);
      if (newNodes.length > 0 || value.trim() === '') {
        nodes = newNodes.length > 0 ? newNodes : [createNode()];
        collapsedNodes = buildCollapsedSet(nodes);
      }
    }
  }

  function emitChange() {
    const serialized = serialiseOutliner(nodes);
    if (serialized !== value) {
      onChange(serialized);
    }
  }

  function handleContentChange(event: CustomEvent<{ id: string; content: string }>) {
    const { id, content } = event.detail;
    const found = findNodeById(nodes, id);
    if (found) {
      found.node.content = content;
      nodes = [...nodes]; // Trigger reactivity
      emitChange();
    }
  }

  function handleToggleCollapse(event: CustomEvent<{ id: string }>) {
    const { id } = event.detail;
    const found = findNodeById(nodes, id);
    if (found) {
      // Update node's collapsed property for persistence
      found.node.collapsed = !found.node.collapsed;
      if (found.node.collapsed) {
        collapsedNodes.add(id);
      } else {
        collapsedNodes.delete(id);
        found.node.collapsed = undefined; // Don't store false, just remove
      }
      collapsedNodes = new Set(collapsedNodes); // Trigger reactivity
      nodes = [...nodes];
      emitChange();
    }
  }

  // Toolbar functions
  function expandAll() {
    const flat = flattenNodes(nodes);
    for (const node of flat) {
      node.collapsed = undefined;
    }
    collapsedNodes = new Set();
    nodes = [...nodes];
    emitChange();
  }

  function collapseAll() {
    const flat = flattenNodes(nodes);
    const newCollapsed = new Set<string>();
    for (const node of flat) {
      if (node.children.length > 0) {
        node.collapsed = true;
        newCollapsed.add(node.id);
      }
    }
    collapsedNodes = newCollapsed;
    nodes = [...nodes];
    emitChange();
  }

  function expandLevel(level: number) {
    function traverse(nodeList: OutlinerNode[], currentLevel: number) {
      for (const node of nodeList) {
        if (node.children.length > 0) {
          if (currentLevel < level) {
            node.collapsed = undefined;
            collapsedNodes.delete(node.id);
          } else {
            node.collapsed = true;
            collapsedNodes.add(node.id);
          }
        }
        traverse(node.children, currentLevel + 1);
      }
    }
    traverse(nodes, 0);
    collapsedNodes = new Set(collapsedNodes);
    nodes = [...nodes];
    emitChange();
  }

  function handleFocus(_event: CustomEvent<{ id: string }>) {
    // Focus tracking handled by DOM
  }

  function handleBlur(_event: CustomEvent<{ id: string }>) {
    // Blur tracking handled by DOM
  }

  function handleDragStart(event: CustomEvent<{ id: string }>) {
    currentDraggedNodeId = event.detail.id;
  }

  function handleDragEnd() {
    // Reset after a short delay to ensure drop has processed
    setTimeout(() => { currentDraggedNodeId = null; }, 100);
  }

  function handleDrop(event: CustomEvent<{ targetId: string; position: 'before' | 'after' | 'child' }>) {
    const { targetId, position } = event.detail;
    const draggedId = currentDraggedNodeId;
    currentDraggedNodeId = null;

    if (!draggedId || draggedId === targetId) return;

    // Find the dragged node
    const draggedResult = findNodeById(nodes, draggedId);
    if (!draggedResult) return;

    // Check if target is a descendant of dragged (can't drop parent into child)
    function isDescendant(parentId: string, childId: string): boolean {
      const parentResult = findNodeById(nodes, parentId);
      if (!parentResult) return false;
      const flat = flattenNodes([parentResult.node]);
      return flat.some(n => n.id === childId);
    }

    if (isDescendant(draggedId, targetId)) return;

    // Remove the dragged node from its current position
    const { node: draggedNode, parent: draggedParent, index: draggedIndex } = draggedResult;
    draggedParent.splice(draggedIndex, 1);

    // Find the target node (after removal, indices might have shifted)
    const targetResult = findNodeById(nodes, targetId);
    if (!targetResult) {
      // Target not found after removal, restore and abort
      draggedParent.splice(draggedIndex, 0, draggedNode);
      return;
    }

    const { node: targetNode, parent: targetParent, index: targetIndex } = targetResult;

    // Insert at the appropriate position
    if (position === 'before') {
      targetParent.splice(targetIndex, 0, draggedNode);
    } else if (position === 'after') {
      targetParent.splice(targetIndex + 1, 0, draggedNode);
    } else if (position === 'child') {
      targetNode.children.push(draggedNode);
      // Expand the target if collapsed
      collapsedNodes.delete(targetId);
      collapsedNodes = new Set(collapsedNodes);
    }

    nodes = [...nodes];
    emitChange();
  }

  async function focusNode(id: string) {
    await tick();
    // Find the content element and focus it
    const contentEl = containerElement.querySelector(`[data-node-id="${id}"] .content`) as HTMLElement;
    if (contentEl) {
      contentEl.focus();
      // Move cursor to end
      const selection = window.getSelection();
      const range = document.createRange();
      range.selectNodeContents(contentEl);
      range.collapse(false);
      selection?.removeAllRanges();
      selection?.addRange(range);
    }
  }

  async function focusNodeAtStart(id: string) {
    await tick();
    const contentEl = containerElement.querySelector(`[data-node-id="${id}"] .content`) as HTMLElement;
    if (contentEl) {
      contentEl.focus();
      // Move cursor to start
      const selection = window.getSelection();
      const range = document.createRange();
      range.selectNodeContents(contentEl);
      range.collapse(true);
      selection?.removeAllRanges();
      selection?.addRange(range);
    }
  }

  async function handleKeyDown(event: CustomEvent<{ id: string; event: KeyboardEvent; element: HTMLElement }>) {
    const { id, event: keyEvent, element } = event.detail;
    const found = findNodeById(nodes, id);
    if (!found) return;

    const { node, parent, index } = found;

    // Tab - Indent (make child of previous sibling)
    if (keyEvent.key === 'Tab' && !keyEvent.shiftKey) {
      keyEvent.preventDefault();
      if (index > 0) {
        // Remove from current position
        parent.splice(index, 1);
        // Add as last child of previous sibling
        const prevSibling = parent[index - 1];
        prevSibling.children.push(node);
        // Expand the parent if collapsed
        collapsedNodes.delete(prevSibling.id);
        nodes = [...nodes];
        emitChange();
        focusNode(id);
      }
      return;
    }

    // Shift+Tab - Outdent (move up a level)
    if (keyEvent.key === 'Tab' && keyEvent.shiftKey) {
      keyEvent.preventDefault();
      const parentNode = findParentNode(nodes, id);
      if (parentNode) {
        // Remove from current parent
        parent.splice(index, 1);
        // Find parent's position in its parent
        const grandparent = findParentNode(nodes, parentNode.id);
        const grandparentArray = grandparent ? grandparent.children : nodes;
        const parentIndex = grandparentArray.findIndex(n => n.id === parentNode.id);
        // Insert after the parent
        grandparentArray.splice(parentIndex + 1, 0, node);
        nodes = [...nodes];
        emitChange();
        focusNode(id);
      }
      return;
    }

    // Enter - New sibling after current
    if (keyEvent.key === 'Enter' && !keyEvent.shiftKey) {
      keyEvent.preventDefault();
      const newNode = createNode();
      // Get cursor position
      const selection = window.getSelection();
      if (selection && selection.rangeCount > 0) {
        const range = selection.getRangeAt(0);
        const cursorPos = range.startOffset;
        const fullText = element.textContent || '';

        // Split content at cursor
        if (cursorPos < fullText.length) {
          newNode.content = fullText.slice(cursorPos);
          node.content = fullText.slice(0, cursorPos);
        }
      }

      // Insert new node after current
      parent.splice(index + 1, 0, newNode);
      nodes = [...nodes];
      emitChange();
      focusNodeAtStart(newNode.id);
      return;
    }

    // Backspace at start of empty node - Delete node
    if (keyEvent.key === 'Backspace') {
      const selection = window.getSelection();
      if (selection && selection.rangeCount > 0) {
        const range = selection.getRangeAt(0);
        if (range.startOffset === 0 && range.endOffset === 0) {
          // At start of node
          if (node.content === '' && nodes.length > 1 || parent.length > 1) {
            keyEvent.preventDefault();
            // Get previous node to focus
            const prevNode = getPreviousNode(nodes, id);
            // Move children to parent at current position
            if (node.children.length > 0) {
              parent.splice(index, 1, ...node.children);
            } else {
              parent.splice(index, 1);
            }
            // Clean up empty arrays
            if (parent.length === 0 && parent !== nodes) {
              // This shouldn't happen, but safety check
            }
            nodes = [...nodes];
            emitChange();
            if (prevNode) {
              focusNode(prevNode.id);
            } else if (nodes.length > 0) {
              const flat = flattenNodes(nodes);
              if (flat.length > 0) {
                focusNode(flat[0].id);
              }
            }
            return;
          } else if (node.content === '' && nodes.length === 1 && parent.length === 1) {
            // Only one node left, don't delete
            keyEvent.preventDefault();
            return;
          }

          // Merge with previous node if not empty
          const prevNode = getPreviousNode(nodes, id);
          if (prevNode && node.content !== '') {
            keyEvent.preventDefault();
            const prevContent = prevNode.content;
            prevNode.content = prevContent + node.content;
            // Move children
            if (node.children.length > 0) {
              prevNode.children.push(...node.children);
              collapsedNodes.delete(prevNode.id);
            }
            parent.splice(index, 1);
            nodes = [...nodes];
            emitChange();
            // Focus at merge point
            await tick();
            const contentEl = containerElement.querySelector(`[data-node-id="${prevNode.id}"] .content`) as HTMLElement;
            if (contentEl) {
              contentEl.focus();
              const selection = window.getSelection();
              const range = document.createRange();
              // Move cursor to the merge point
              const textNode = contentEl.firstChild;
              if (textNode) {
                range.setStart(textNode, prevContent.length);
                range.collapse(true);
                selection?.removeAllRanges();
                selection?.addRange(range);
              }
            }
            return;
          }
        }
      }
    }

    // Arrow Up - Navigate to previous node
    if (keyEvent.key === 'ArrowUp' && !keyEvent.ctrlKey && !keyEvent.metaKey) {
      const selection = window.getSelection();
      if (selection && selection.rangeCount > 0) {
        const range = selection.getRangeAt(0);
        // Only navigate if at start of content
        if (range.startOffset === 0 && range.endOffset === 0) {
          keyEvent.preventDefault();
          const prevNode = getPreviousNode(nodes, id);
          if (prevNode) {
            focusNode(prevNode.id);
          }
          return;
        }
      }
    }

    // Arrow Down - Navigate to next node
    if (keyEvent.key === 'ArrowDown' && !keyEvent.ctrlKey && !keyEvent.metaKey) {
      const selection = window.getSelection();
      if (selection && selection.rangeCount > 0) {
        const range = selection.getRangeAt(0);
        const fullText = element.textContent || '';
        // Only navigate if at end of content
        if (range.startOffset === fullText.length && range.endOffset === fullText.length) {
          keyEvent.preventDefault();
          // Skip collapsed children
          const nextNode = getNextVisibleNode(nodes, id, collapsedNodes);
          if (nextNode) {
            focusNodeAtStart(nextNode.id);
          }
          return;
        }
      }
    }

    // Ctrl/Cmd + Arrow Up - Move node up
    if (keyEvent.key === 'ArrowUp' && (keyEvent.ctrlKey || keyEvent.metaKey)) {
      keyEvent.preventDefault();
      if (index > 0) {
        // Swap with previous sibling
        [parent[index - 1], parent[index]] = [parent[index], parent[index - 1]];
        nodes = [...nodes];
        emitChange();
        focusNode(id);
      }
      return;
    }

    // Ctrl/Cmd + Arrow Down - Move node down
    if (keyEvent.key === 'ArrowDown' && (keyEvent.ctrlKey || keyEvent.metaKey)) {
      keyEvent.preventDefault();
      if (index < parent.length - 1) {
        // Swap with next sibling
        [parent[index], parent[index + 1]] = [parent[index + 1], parent[index]];
        nodes = [...nodes];
        emitChange();
        focusNode(id);
      }
      return;
    }
  }

  // Get next visible node (respecting collapsed state)
  function getNextVisibleNode(
    allNodes: OutlinerNode[],
    currentId: string,
    collapsed: Set<string>
  ): OutlinerNode | undefined {
    const flat: OutlinerNode[] = [];

    function flatten(nodesToFlatten: OutlinerNode[]): void {
      for (const node of nodesToFlatten) {
        flat.push(node);
        if (!collapsed.has(node.id)) {
          flatten(node.children);
        }
      }
    }

    flatten(allNodes);
    const index = flat.findIndex(n => n.id === currentId);
    if (index >= 0 && index < flat.length - 1) {
      return flat[index + 1];
    }
    return undefined;
  }

  // Export focus method
  export function focus() {
    if (nodes.length > 0) {
      focusNode(nodes[0].id);
    }
  }
</script>

<div class="outliner-container" class:dark={isDark}>
  <!-- Toolbar -->
  {#if showToolbar && !readonly}
    <div class="outliner-toolbar" class:dark={isDark}>
      <button
        type="button"
        class="toolbar-btn"
        on:click={expandAll}
        title={$_('outliner.expandAll')}
      >
        <svg viewBox="0 0 20 20" fill="currentColor" class="icon">
          <path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd" />
        </svg>
        <span>{$_('outliner.expandAll')}</span>
      </button>
      <button
        type="button"
        class="toolbar-btn"
        on:click={collapseAll}
        title={$_('outliner.collapseAll')}
      >
        <svg viewBox="0 0 20 20" fill="currentColor" class="icon">
          <path fill-rule="evenodd" d="M7.293 14.707a1 1 0 010-1.414L10.586 10 7.293 6.707a1 1 0 011.414-1.414l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0z" clip-rule="evenodd" />
        </svg>
        <span>{$_('outliner.collapseAll')}</span>
      </button>
      <div class="toolbar-separator"></div>
      <button
        type="button"
        class="toolbar-btn"
        on:click={() => expandLevel(1)}
        title={$_('outliner.level', { values: { n: 1 } })}
      >
        L1
      </button>
      <button
        type="button"
        class="toolbar-btn"
        on:click={() => expandLevel(2)}
        title={$_('outliner.level', { values: { n: 2 } })}
      >
        L2
      </button>
      <button
        type="button"
        class="toolbar-btn"
        on:click={() => expandLevel(3)}
        title={$_('outliner.level', { values: { n: 3 } })}
      >
        L3
      </button>
    </div>
  {/if}

  <!-- Editor content -->
  <div
    bind:this={containerElement}
    class="outliner-editor"
    class:dark={isDark}
    style="font-size: {fontSize}px"
  >
    {#if nodes.length === 0}
      <div class="empty-state">
        {$_('outliner.emptyHint')}
      </div>
    {:else}
      {#each nodes as node (node.id)}
        <OutlinerNodeComponent
          {node}
          depth={0}
          collapsed={collapsedNodes.has(node.id)}
          {collapsedNodes}
          {readonly}
          {isDark}
          on:contentChange={handleContentChange}
          on:toggleCollapse={handleToggleCollapse}
          on:keydown={handleKeyDown}
          on:focus={handleFocus}
          on:blur={handleBlur}
          on:dragStart={handleDragStart}
          on:dragEnd={handleDragEnd}
          on:drop={handleDrop}
        />
      {/each}
    {/if}
  </div>
</div>

<style>
  .outliner-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: white;
  }

  .outliner-container.dark {
    background: rgb(17 24 39);
  }

  .outliner-toolbar {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.5rem;
    border-bottom: 1px solid #e5e7eb;
    background: #f9fafb;
    flex-shrink: 0;
  }

  .outliner-toolbar.dark {
    background: rgb(31 41 55);
    border-bottom-color: rgb(55 65 81);
  }

  .toolbar-btn {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.25rem 0.5rem;
    border: none;
    background: transparent;
    color: #6b7280;
    font-size: 0.75rem;
    border-radius: 0.25rem;
    cursor: pointer;
    transition: background-color 0.15s, color 0.15s;
  }

  .toolbar-btn:hover {
    background: #e5e7eb;
    color: #374151;
  }

  .dark .toolbar-btn {
    color: #9ca3af;
  }

  .dark .toolbar-btn:hover {
    background: rgb(55 65 81);
    color: #e5e7eb;
  }

  .toolbar-btn .icon {
    width: 1rem;
    height: 1rem;
  }

  .toolbar-separator {
    width: 1px;
    height: 1rem;
    background: #d1d5db;
    margin: 0 0.25rem;
  }

  .dark .toolbar-separator {
    background: rgb(75 85 99);
  }

  .outliner-editor {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 1rem;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    line-height: 1.6;
  }

  .outliner-editor.dark {
    color: rgb(243 244 246); /* gray-100 */
  }

  .empty-state {
    color: #9ca3af;
    padding: 1rem;
    text-align: center;
  }

  .dark .empty-state {
    color: #6b7280;
  }
</style>
