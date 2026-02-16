/**
 * Outliner editor bundle for iOS WKWebView.
 *
 * Vanilla JS/DOM tree editor that serialises to the same markdown-compatible
 * format as the web app's outliner (- expanded / + collapsed items, 2-space indent).
 * Ported from outlinerParser.ts (framework-agnostic).
 */

import { postToSwift } from '../bridge';

// ---- Types ----

interface OutlinerNode {
  id: string;
  content: string;
  children: OutlinerNode[];
  collapsed?: boolean;
}

// ---- Parser (ported from outlinerParser.ts) ----

const INDENT_SIZE = 2;

function generateId(): string {
  return crypto.randomUUID();
}

function parseOutliner(text: string): OutlinerNode[] {
  if (!text || text.trim() === '') return [];

  const lines = text.split('\n');
  const root: OutlinerNode[] = [];
  const stack: { node: OutlinerNode; indent: number }[] = [];

  for (const line of lines) {
    if (line.trim() === '') continue;

    let spaces = 0;
    for (const char of line) {
      if (char === ' ') spaces++;
      else if (char === '\t') spaces += INDENT_SIZE;
      else break;
    }
    const level = Math.floor(spaces / INDENT_SIZE);
    let content = line.slice(spaces);

    let collapsed = false;
    if (content.startsWith('+ ')) {
      collapsed = true;
      content = content.slice(2);
    } else if (content.startsWith('- ')) {
      content = content.slice(2);
    } else if (content.startsWith('>')) {
      collapsed = true;
      content = content.slice(1);
    } else if (content.endsWith(' [-]')) {
      collapsed = true;
      content = content.slice(0, -4);
    }

    const node: OutlinerNode = {
      id: generateId(),
      content,
      children: [],
      collapsed: collapsed || undefined,
    };

    if (level === 0) {
      root.push(node);
      stack.length = 0;
      stack.push({ node, indent: 0 });
    } else {
      while (stack.length > 0 && stack[stack.length - 1].indent >= level) {
        stack.pop();
      }
      if (stack.length === 0) {
        root.push(node);
        stack.push({ node, indent: 0 });
      } else {
        stack[stack.length - 1].node.children.push(node);
        stack.push({ node, indent: level });
      }
    }
  }

  return root;
}

function serialiseOutliner(nodes: OutlinerNode[]): string {
  const lines: string[] = [];

  function serialiseNode(node: OutlinerNode, level: number): void {
    const indent = ' '.repeat(level * INDENT_SIZE);
    const bullet = node.collapsed ? '+ ' : '- ';
    lines.push(indent + bullet + node.content);
    for (const child of node.children) {
      serialiseNode(child, level + 1);
    }
  }

  for (const node of nodes) {
    serialiseNode(node, 0);
  }

  return lines.join('\n');
}

// ---- Utility ----

function findNodeById(
  nodes: OutlinerNode[],
  id: string,
): { node: OutlinerNode; parent: OutlinerNode[]; index: number } | undefined {
  for (let i = 0; i < nodes.length; i++) {
    if (nodes[i].id === id) return { node: nodes[i], parent: nodes, index: i };
    const result = findNodeById(nodes[i].children, id);
    if (result) return result;
  }
  return undefined;
}

function findParentNode(nodes: OutlinerNode[], id: string): OutlinerNode | undefined {
  for (const node of nodes) {
    for (const child of node.children) {
      if (child.id === id) return node;
    }
    const result = findParentNode(node.children, id);
    if (result) return result;
  }
  return undefined;
}

function flattenNodes(nodes: OutlinerNode[]): OutlinerNode[] {
  const result: OutlinerNode[] = [];
  for (const node of nodes) {
    result.push(node);
    result.push(...flattenNodes(node.children));
  }
  return result;
}

function flattenVisible(nodes: OutlinerNode[]): OutlinerNode[] {
  const result: OutlinerNode[] = [];
  for (const node of nodes) {
    result.push(node);
    if (!node.collapsed) {
      result.push(...flattenVisible(node.children));
    }
  }
  return result;
}

// ---- State ----

let tree: OutlinerNode[] = [];
let isDark = false;
let focusedId: string | null = null;
let draggedId: string | null = null;
let dropTargetId: string | null = null;
let dropPosition: 'before' | 'after' | 'child' = 'after';

// ---- Content changed notification ----

function notifyChanged(): void {
  postToSwift({ type: 'contentChanged', content: serialiseOutliner(tree) });
}

// ---- Rendering ----

function render(): void {
  const container = document.getElementById('outliner')!;
  container.innerHTML = '';
  for (const node of tree) {
    container.appendChild(renderNode(node, 0));
  }
}

function renderNode(node: OutlinerNode, depth: number): HTMLElement {
  const row = document.createElement('div');
  row.className = 'outliner-row';
  row.dataset.id = node.id;
  row.draggable = true;
  row.style.paddingLeft = `${depth * 20 + 8}px`;

  // Drop indicator
  if (dropTargetId === node.id) {
    row.classList.add(`drop-${dropPosition}`);
  }

  // Toggle button (expand/collapse)
  const toggle = document.createElement('span');
  toggle.className = 'outliner-toggle';
  if (node.children.length > 0) {
    toggle.textContent = node.collapsed ? '\u25B6' : '\u25BC';
    toggle.addEventListener('click', (e) => {
      e.stopPropagation();
      node.collapsed = !node.collapsed;
      notifyChanged();
      render();
      focusNode(node.id);
    });
  } else {
    toggle.textContent = '\u2022';
    toggle.classList.add('leaf');
  }
  row.appendChild(toggle);

  // Content (contenteditable)
  const contentEl = document.createElement('div');
  contentEl.className = 'outliner-content';
  contentEl.contentEditable = 'true';
  contentEl.textContent = node.content;
  contentEl.dataset.id = node.id;

  if (focusedId === node.id) {
    requestAnimationFrame(() => {
      contentEl.focus();
      // Place cursor at end
      const range = document.createRange();
      const sel = window.getSelection();
      if (contentEl.childNodes.length > 0) {
        range.selectNodeContents(contentEl);
        range.collapse(false);
      }
      sel?.removeAllRanges();
      sel?.addRange(range);
    });
  }

  // Input handler
  contentEl.addEventListener('input', () => {
    node.content = contentEl.textContent || '';
    notifyChanged();
  });

  // Focus tracking
  contentEl.addEventListener('focus', () => {
    focusedId = node.id;
  });

  // Keyboard handler
  contentEl.addEventListener('keydown', (e) => {
    handleKeyDown(e, node);
  });

  row.appendChild(contentEl);

  // Drag and drop
  row.addEventListener('dragstart', (e) => {
    draggedId = node.id;
    e.dataTransfer!.effectAllowed = 'move';
    row.classList.add('dragging');
  });

  row.addEventListener('dragend', () => {
    draggedId = null;
    dropTargetId = null;
    dropPosition = 'after';
    document.querySelectorAll('.dragging').forEach((el) => el.classList.remove('dragging'));
    render();
  });

  row.addEventListener('dragover', (e) => {
    e.preventDefault();
    if (draggedId === node.id) return;
    e.dataTransfer!.dropEffect = 'move';

    dropTargetId = node.id;
    const rect = row.getBoundingClientRect();
    const y = e.clientY - rect.top;
    const third = rect.height / 3;
    if (y < third) dropPosition = 'before';
    else if (y > third * 2) dropPosition = 'child';
    else dropPosition = 'after';

    render();
  });

  row.addEventListener('dragleave', () => {
    if (dropTargetId === node.id) {
      dropTargetId = null;
      render();
    }
  });

  row.addEventListener('drop', (e) => {
    e.preventDefault();
    if (!draggedId || draggedId === node.id) return;
    moveNode(draggedId, node.id, dropPosition);
    draggedId = null;
    dropTargetId = null;
    notifyChanged();
    render();
  });

  // Container for row + children
  const wrapper = document.createElement('div');
  wrapper.className = 'outliner-node';
  wrapper.appendChild(row);

  // Render children (unless collapsed)
  if (!node.collapsed && node.children.length > 0) {
    for (const child of node.children) {
      wrapper.appendChild(renderNode(child, depth + 1));
    }
  }

  return wrapper;
}

// ---- Focus helpers ----

function focusNode(id: string): void {
  focusedId = id;
  requestAnimationFrame(() => {
    const el = document.querySelector<HTMLElement>(`.outliner-content[data-id="${id}"]`);
    if (el) {
      el.focus();
      const range = document.createRange();
      const sel = window.getSelection();
      if (el.childNodes.length > 0) {
        range.selectNodeContents(el);
        range.collapse(false);
      }
      sel?.removeAllRanges();
      sel?.addRange(range);
    }
  });
}

// ---- Keyboard handling ----

function handleKeyDown(e: KeyboardEvent, node: OutlinerNode): void {
  if (e.key === 'Enter') {
    e.preventDefault();
    // Create a new sibling after the current node
    const newNode: OutlinerNode = { id: generateId(), content: '', children: [] };
    const found = findNodeById(tree, node.id);
    if (found) {
      found.parent.splice(found.index + 1, 0, newNode);
      notifyChanged();
      render();
      focusNode(newNode.id);
    }
    return;
  }

  if (e.key === 'Backspace' && (node.content === '' || node.content === null)) {
    e.preventDefault();
    // Delete empty node, focus previous
    const flat = flattenVisible(tree);
    const idx = flat.findIndex((n) => n.id === node.id);
    const prevNode = idx > 0 ? flat[idx - 1] : null;

    const found = findNodeById(tree, node.id);
    if (found && flat.length > 1) {
      // Move children up to parent level before removing
      for (const child of node.children) {
        found.parent.splice(found.index + 1, 0, child);
      }
      found.parent.splice(found.index, 1);
      notifyChanged();
      render();
      if (prevNode) focusNode(prevNode.id);
    }
    return;
  }

  if (e.key === 'Tab') {
    e.preventDefault();
    if (e.shiftKey) {
      // Outdent: move node to be a sibling of its parent
      outdentNode(node);
    } else {
      // Indent: make node a child of the previous sibling
      indentNode(node);
    }
    notifyChanged();
    render();
    focusNode(node.id);
    return;
  }

  if (e.key === 'ArrowUp' && !e.altKey) {
    const flat = flattenVisible(tree);
    const idx = flat.findIndex((n) => n.id === node.id);
    if (idx > 0) {
      e.preventDefault();
      focusNode(flat[idx - 1].id);
    }
    return;
  }

  if (e.key === 'ArrowDown' && !e.altKey) {
    const flat = flattenVisible(tree);
    const idx = flat.findIndex((n) => n.id === node.id);
    if (idx < flat.length - 1) {
      e.preventDefault();
      focusNode(flat[idx + 1].id);
    }
    return;
  }
}

// ---- Tree operations ----

function indentNode(node: OutlinerNode): void {
  const found = findNodeById(tree, node.id);
  if (!found || found.index === 0) return;

  // Previous sibling becomes parent
  const prevSibling = found.parent[found.index - 1];
  found.parent.splice(found.index, 1);
  prevSibling.children.push(node);
  // Ensure parent is expanded
  prevSibling.collapsed = undefined;
}

function outdentNode(node: OutlinerNode): void {
  const parent = findParentNode(tree, node.id);
  if (!parent) return; // Already at root level

  const grandparent = findNodeById(tree, parent.id);
  if (!grandparent) return;

  // Remove from parent's children
  const childIndex = parent.children.indexOf(node);
  if (childIndex === -1) return;

  // Move any siblings after this node to become children of this node
  const trailingChildren = parent.children.splice(childIndex + 1);
  parent.children.splice(childIndex, 1);
  node.children.push(...trailingChildren);

  // Insert after parent in grandparent's children
  grandparent.parent.splice(grandparent.index + 1, 0, node);
}

function moveNode(fromId: string, toId: string, position: 'before' | 'after' | 'child'): void {
  const from = findNodeById(tree, fromId);
  const to = findNodeById(tree, toId);
  if (!from || !to) return;

  // Prevent moving a node into its own descendants
  const descendants = flattenNodes([from.node]);
  if (descendants.some((n) => n.id === toId)) return;

  // Remove from current position
  from.parent.splice(from.index, 1);

  // Re-find target (indices may have shifted)
  const toAfter = findNodeById(tree, toId);
  if (!toAfter) return;

  if (position === 'child') {
    toAfter.node.children.push(from.node);
    toAfter.node.collapsed = undefined;
  } else if (position === 'before') {
    toAfter.parent.splice(toAfter.index, 0, from.node);
  } else {
    toAfter.parent.splice(toAfter.index + 1, 0, from.node);
  }
}

// ---- Theme ----

function applyTheme(dark: boolean): void {
  isDark = dark;
  document.documentElement.classList.toggle('dark', dark);
}

// ---- Bridge API ----

window.bridge = {
  setContent(content: string, dark: boolean): void {
    applyTheme(dark);
    tree = parseOutliner(content);
    // Ensure at least one node
    if (tree.length === 0) {
      tree.push({ id: generateId(), content: '', children: [] });
    }
    render();
    // Focus first node
    if (tree.length > 0) {
      focusNode(tree[0].id);
    }
  },

  setTheme(dark: boolean): void {
    applyTheme(dark);
  },

  resolveAttachment(): void {
    // Not used by outliner
  },
};

// ---- Styles ----

const style = document.createElement('style');
style.textContent = `
  * { box-sizing: border-box; margin: 0; padding: 0; }
  :root {
    --bg: #ffffff;
    --fg: #1c1c1e;
    --row-hover: #f2f2f7;
    --border: #e5e5ea;
    --toggle: #8e8e93;
    --focus-ring: #007aff;
    --drop-indicator: #007aff;
  }
  .dark {
    --bg: #1c1c1e;
    --fg: #e5e5e7;
    --row-hover: #2c2c2e;
    --border: #38383a;
    --toggle: #8e8e93;
    --focus-ring: #64d2ff;
    --drop-indicator: #64d2ff;
  }
  html, body {
    height: 100%;
    background: var(--bg);
    color: var(--fg);
    font-family: -apple-system, BlinkMacSystemFont, sans-serif;
    font-size: 16px;
    line-height: 1.5;
    -webkit-text-size-adjust: 100%;
    overflow-y: auto;
    -webkit-overflow-scrolling: touch;
  }
  #outliner {
    padding: 8px 0;
    min-height: 100%;
  }
  .outliner-row {
    display: flex;
    align-items: flex-start;
    padding-right: 12px;
    padding-top: 4px;
    padding-bottom: 4px;
    cursor: default;
    border-radius: 4px;
    transition: background-color 0.1s;
  }
  .outliner-row:hover {
    background: var(--row-hover);
  }
  .outliner-row.dragging {
    opacity: 0.4;
  }
  .outliner-row.drop-before {
    border-top: 2px solid var(--drop-indicator);
  }
  .outliner-row.drop-after {
    border-bottom: 2px solid var(--drop-indicator);
  }
  .outliner-row.drop-child {
    background: color-mix(in srgb, var(--drop-indicator) 15%, transparent);
  }
  .outliner-toggle {
    flex-shrink: 0;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 10px;
    color: var(--toggle);
    cursor: pointer;
    user-select: none;
    -webkit-user-select: none;
  }
  .outliner-toggle.leaf {
    font-size: 8px;
    cursor: default;
  }
  .outliner-content {
    flex: 1;
    min-height: 24px;
    padding: 2px 4px;
    outline: none;
    word-wrap: break-word;
    overflow-wrap: break-word;
    border-radius: 3px;
  }
  .outliner-content:focus {
    box-shadow: 0 0 0 2px var(--focus-ring);
  }
`;
document.head.appendChild(style);

// ---- Init ----

postToSwift({ type: 'ready' });
