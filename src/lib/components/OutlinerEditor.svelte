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

  // Font size from settings
  $: fontSize = getFontSize($settings.fontSize);

  // Parse value on mount and when value changes externally
  onMount(() => {
    nodes = parseOutliner(value);
    // If empty, create a default node
    if (nodes.length === 0) {
      nodes = [createNode()];
      emitChange();
    }
  });

  // Handle external value changes
  $: {
    const currentSerialized = serialiseOutliner(nodes);
    if (value !== currentSerialized && value !== undefined) {
      const newNodes = parseOutliner(value);
      if (newNodes.length > 0 || value.trim() === '') {
        nodes = newNodes.length > 0 ? newNodes : [createNode()];
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
    if (collapsedNodes.has(id)) {
      collapsedNodes.delete(id);
    } else {
      collapsedNodes.add(id);
    }
    collapsedNodes = new Set(collapsedNodes); // Trigger reactivity
  }

  function handleFocus(_event: CustomEvent<{ id: string }>) {
    // Focus tracking handled by DOM
  }

  function handleBlur(_event: CustomEvent<{ id: string }>) {
    // Blur tracking handled by DOM
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
        {readonly}
        {isDark}
        on:contentChange={handleContentChange}
        on:toggleCollapse={handleToggleCollapse}
        on:keydown={handleKeyDown}
        on:focus={handleFocus}
        on:blur={handleBlur}
      />
    {/each}
  {/if}
</div>

<style>
  .outliner-editor {
    height: 100%;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 1rem;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    line-height: 1.6;
    background: white;
  }

  .outliner-editor.dark {
    background: rgb(17 24 39); /* gray-900 */
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
