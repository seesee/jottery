/**
 * Parser and serialiser for the Outliner document format
 *
 * Content storage format: Tab-indented text with optional collapse marker
 * - Tab character for each indent level
 * - '>' prefix marks collapsed nodes
 * - Human-readable, works with encryption
 * - Can edit as plain text if needed
 *
 * Example:
 *   Item 1
 *   	Child 1.1
 *   	Child 1.2
 *   		Grandchild 1.2.1
 *   >Item 2 (collapsed)
 *   	Hidden child
 */

import type { OutlinerNode } from '../types/outliner';

const INDENT_CHAR = '\t';
const COLLAPSED_PREFIX = '>';

/**
 * Generate a UUID for new nodes
 */
function generateId(): string {
  return crypto.randomUUID();
}

/**
 * Parse indented text into a tree of OutlinerNode objects
 *
 * @param text - The indented text to parse
 * @returns Array of root-level OutlinerNode objects
 */
export function parseOutliner(text: string): OutlinerNode[] {
  if (!text || text.trim() === '') {
    return [];
  }

  const lines = text.split('\n');
  const root: OutlinerNode[] = [];

  // Stack to track parent nodes at each indent level
  // stack[0] = root, stack[1] = indent level 1, etc.
  const stack: { node: OutlinerNode; indent: number }[] = [];

  for (const line of lines) {
    // Skip completely empty lines
    if (line.trim() === '') {
      continue;
    }

    // Calculate indent level (count leading tabs, or fall back to 2-space units for backwards compat)
    let level = 0;
    let charIndex = 0;
    for (const char of line) {
      if (char === '\t') {
        level++;
        charIndex++;
      } else if (char === ' ') {
        // Count spaces for backwards compatibility (2 spaces = 1 level)
        let spaces = 0;
        for (let i = charIndex; i < line.length && line[i] === ' '; i++) {
          spaces++;
        }
        level += Math.floor(spaces / 2);
        charIndex += spaces;
        break;
      } else {
        break;
      }
    }
    let content = line.slice(charIndex);

    // Check for collapsed marker (> prefix)
    let collapsed = false;
    if (content.startsWith(COLLAPSED_PREFIX)) {
      collapsed = true;
      content = content.slice(COLLAPSED_PREFIX.length);
    }
    // Also support old [-] suffix for backwards compatibility
    if (content.endsWith(' [-]')) {
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
      // Root level node
      root.push(node);
      stack.length = 0;
      stack.push({ node, indent: 0 });
    } else {
      // Find the appropriate parent
      while (stack.length > 0 && stack[stack.length - 1].indent >= level) {
        stack.pop();
      }

      if (stack.length === 0) {
        // No valid parent, make it a root node
        root.push(node);
        stack.push({ node, indent: 0 });
      } else {
        // Add as child of the current parent
        const parent = stack[stack.length - 1];
        parent.node.children.push(node);
        stack.push({ node, indent: level });
      }
    }
  }

  return root;
}

/**
 * Serialise a tree of OutlinerNode objects back to indented text
 *
 * @param nodes - Array of root-level OutlinerNode objects
 * @returns Indented text representation
 */
export function serialiseOutliner(nodes: OutlinerNode[]): string {
  const lines: string[] = [];

  function serialiseNode(node: OutlinerNode, level: number): void {
    const indent = INDENT_CHAR.repeat(level);
    const collapsedPrefix = node.collapsed ? COLLAPSED_PREFIX : '';
    lines.push(indent + collapsedPrefix + node.content);
    for (const child of node.children) {
      serialiseNode(child, level + 1);
    }
  }

  for (const node of nodes) {
    serialiseNode(node, 0);
  }

  return lines.join('\n');
}

/**
 * Find a node by ID in the tree
 *
 * @param nodes - Array of root-level nodes to search
 * @param id - The ID to find
 * @returns The found node and its parent array, or undefined
 */
export function findNodeById(
  nodes: OutlinerNode[],
  id: string
): { node: OutlinerNode; parent: OutlinerNode[]; index: number } | undefined {
  function search(
    searchNodes: OutlinerNode[]
  ): { node: OutlinerNode; parent: OutlinerNode[]; index: number } | undefined {
    for (let i = 0; i < searchNodes.length; i++) {
      const node = searchNodes[i];
      if (node.id === id) {
        return { node, parent: searchNodes, index: i };
      }
      const result = search(node.children);
      if (result) {
        return result;
      }
    }
    return undefined;
  }

  return search(nodes);
}

/**
 * Find the parent of a node by the node's ID
 *
 * @param nodes - Array of root-level nodes to search
 * @param id - The ID of the node to find the parent for
 * @returns The parent node, or undefined if the node is at root level or not found
 */
export function findParentNode(
  nodes: OutlinerNode[],
  id: string
): OutlinerNode | undefined {
  function search(
    searchNodes: OutlinerNode[],
    parent: OutlinerNode | undefined
  ): OutlinerNode | undefined {
    for (const node of searchNodes) {
      if (node.id === id) {
        return parent;
      }
      const result = search(node.children, node);
      if (result !== undefined) {
        return result;
      }
    }
    return undefined;
  }

  return search(nodes, undefined);
}

/**
 * Get a flat list of all nodes in document order
 *
 * @param nodes - Array of root-level nodes
 * @returns Flat array of all nodes in document order
 */
export function flattenNodes(nodes: OutlinerNode[]): OutlinerNode[] {
  const result: OutlinerNode[] = [];

  function flatten(nodesToFlatten: OutlinerNode[]): void {
    for (const node of nodesToFlatten) {
      result.push(node);
      flatten(node.children);
    }
  }

  flatten(nodes);
  return result;
}

/**
 * Get the previous node in document order
 *
 * @param nodes - Array of root-level nodes
 * @param id - The ID of the current node
 * @returns The previous node, or undefined if at the start
 */
export function getPreviousNode(
  nodes: OutlinerNode[],
  id: string
): OutlinerNode | undefined {
  const flat = flattenNodes(nodes);
  const index = flat.findIndex(n => n.id === id);
  if (index > 0) {
    return flat[index - 1];
  }
  return undefined;
}

/**
 * Get the next node in document order
 *
 * @param nodes - Array of root-level nodes
 * @param id - The ID of the current node
 * @returns The next node, or undefined if at the end
 */
export function getNextNode(
  nodes: OutlinerNode[],
  id: string
): OutlinerNode | undefined {
  const flat = flattenNodes(nodes);
  const index = flat.findIndex(n => n.id === id);
  if (index >= 0 && index < flat.length - 1) {
    return flat[index + 1];
  }
  return undefined;
}

/**
 * Create a new empty node
 *
 * @returns A new empty OutlinerNode
 */
export function createNode(content: string = ''): OutlinerNode {
  return {
    id: generateId(),
    content,
    children: [],
  };
}

/**
 * Deep clone a node tree
 *
 * @param nodes - Nodes to clone
 * @param regenerateIds - Whether to generate new IDs for the cloned nodes
 * @returns Cloned nodes
 */
export function cloneNodes(nodes: OutlinerNode[], regenerateIds: boolean = false): OutlinerNode[] {
  return nodes.map(node => ({
    id: regenerateIds ? generateId() : node.id,
    content: node.content,
    children: cloneNodes(node.children, regenerateIds),
  }));
}
