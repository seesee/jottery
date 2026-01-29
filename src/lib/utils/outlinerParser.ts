/**
 * Parser and serialiser for the Outliner document format
 *
 * Content storage format: Plain indented text
 * - Two spaces per indent level
 * - Human-readable, works with encryption
 * - Can edit as plain text if needed
 * - Optional [-] suffix marks collapsed nodes
 *
 * Example:
 *   Item 1 [-]
 *     Child 1.1
 *     Child 1.2
 *       Grandchild 1.2.1
 *   Item 2
 */

import type { OutlinerNode } from '../types/outliner';

const INDENT_SIZE = 2;
const INDENT_CHAR = ' ';
const COLLAPSED_MARKER = ' [-]';

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

    // Calculate indent level
    let indent = 0;
    for (const char of line) {
      if (char === INDENT_CHAR) {
        indent++;
      } else {
        break;
      }
    }
    const level = Math.floor(indent / INDENT_SIZE);
    let content = line.slice(indent);

    // Check for collapsed marker
    let collapsed = false;
    if (content.endsWith(COLLAPSED_MARKER)) {
      collapsed = true;
      content = content.slice(0, -COLLAPSED_MARKER.length);
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
      while (stack.length > 0 && stack[stack.length - 1].indent >= level * INDENT_SIZE) {
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
        stack.push({ node, indent: level * INDENT_SIZE });
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
    const indent = INDENT_CHAR.repeat(level * INDENT_SIZE);
    const collapsedSuffix = node.collapsed ? COLLAPSED_MARKER : '';
    lines.push(indent + node.content + collapsedSuffix);
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
