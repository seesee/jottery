import { describe, test, expect } from 'vitest';
import {
  parseOutliner,
  serialiseOutliner,
  findNodeById,
  findParentNode,
  getPreviousNode,
  getNextNode,
  createNode,
  cloneNodes,
  flattenNodes,
} from './outlinerParser';
import type { OutlinerNode } from '../types/outliner';

describe('parseOutliner', () => {
  test('parses empty string', () => {
    expect(parseOutliner('')).toEqual([]);
    expect(parseOutliner('   ')).toEqual([]);
    expect(parseOutliner('\n\n')).toEqual([]);
  });

  test('parses single line', () => {
    const result = parseOutliner('Item 1');
    expect(result).toHaveLength(1);
    expect(result[0].content).toBe('Item 1');
    expect(result[0].children).toHaveLength(0);
  });

  test('parses multiple root items', () => {
    const result = parseOutliner('Item 1\nItem 2\nItem 3');
    expect(result).toHaveLength(3);
    expect(result[0].content).toBe('Item 1');
    expect(result[1].content).toBe('Item 2');
    expect(result[2].content).toBe('Item 3');
  });

  test('parses nested items', () => {
    const text = `Item 1
  Child 1.1
  Child 1.2
Item 2`;
    const result = parseOutliner(text);
    expect(result).toHaveLength(2);
    expect(result[0].content).toBe('Item 1');
    expect(result[0].children).toHaveLength(2);
    expect(result[0].children[0].content).toBe('Child 1.1');
    expect(result[0].children[1].content).toBe('Child 1.2');
    expect(result[1].content).toBe('Item 2');
    expect(result[1].children).toHaveLength(0);
  });

  test('parses deeply nested items', () => {
    const text = `Item 1
  Child 1.1
    Grandchild 1.1.1
      Great-grandchild
    Grandchild 1.1.2
  Child 1.2`;
    const result = parseOutliner(text);
    expect(result).toHaveLength(1);
    expect(result[0].children).toHaveLength(2);
    expect(result[0].children[0].children).toHaveLength(2);
    expect(result[0].children[0].children[0].content).toBe('Grandchild 1.1.1');
    expect(result[0].children[0].children[0].children).toHaveLength(1);
    expect(result[0].children[0].children[0].children[0].content).toBe('Great-grandchild');
  });

  test('skips empty lines', () => {
    const text = `Item 1

Item 2

  Child 2.1`;
    const result = parseOutliner(text);
    expect(result).toHaveLength(2);
    expect(result[1].children).toHaveLength(1);
  });

  test('handles irregular indentation', () => {
    // Odd number of spaces should still work
    const text = `Item 1
   Child (3 spaces)`;
    const result = parseOutliner(text);
    expect(result).toHaveLength(1);
    expect(result[0].children).toHaveLength(1);
  });
});

describe('serialiseOutliner', () => {
  test('serialises empty array', () => {
    expect(serialiseOutliner([])).toBe('');
  });

  test('serialises single node', () => {
    const nodes: OutlinerNode[] = [
      { id: '1', content: 'Item 1', children: [] },
    ];
    expect(serialiseOutliner(nodes)).toBe('Item 1');
  });

  test('serialises multiple root nodes', () => {
    const nodes: OutlinerNode[] = [
      { id: '1', content: 'Item 1', children: [] },
      { id: '2', content: 'Item 2', children: [] },
    ];
    expect(serialiseOutliner(nodes)).toBe('Item 1\nItem 2');
  });

  test('serialises nested nodes', () => {
    const nodes: OutlinerNode[] = [
      {
        id: '1',
        content: 'Item 1',
        children: [
          { id: '2', content: 'Child 1.1', children: [] },
          { id: '3', content: 'Child 1.2', children: [] },
        ],
      },
    ];
    expect(serialiseOutliner(nodes)).toBe('Item 1\n  Child 1.1\n  Child 1.2');
  });

  test('serialises deeply nested nodes', () => {
    const nodes: OutlinerNode[] = [
      {
        id: '1',
        content: 'Item 1',
        children: [
          {
            id: '2',
            content: 'Child 1.1',
            children: [
              { id: '3', content: 'Grandchild', children: [] },
            ],
          },
        ],
      },
    ];
    expect(serialiseOutliner(nodes)).toBe('Item 1\n  Child 1.1\n    Grandchild');
  });

  test('round-trip: parse then serialise produces equivalent output', () => {
    const original = `Item 1
  Child 1.1
    Grandchild 1.1.1
  Child 1.2
Item 2
  Child 2.1`;
    const parsed = parseOutliner(original);
    const serialised = serialiseOutliner(parsed);
    expect(serialised).toBe(original);
  });
});

describe('findNodeById', () => {
  const nodes: OutlinerNode[] = [
    {
      id: 'root1',
      content: 'Root 1',
      children: [
        { id: 'child1', content: 'Child 1', children: [] },
        {
          id: 'child2',
          content: 'Child 2',
          children: [
            { id: 'grandchild', content: 'Grandchild', children: [] },
          ],
        },
      ],
    },
    { id: 'root2', content: 'Root 2', children: [] },
  ];

  test('finds root node', () => {
    const result = findNodeById(nodes, 'root1');
    expect(result).toBeDefined();
    expect(result?.node.content).toBe('Root 1');
    expect(result?.index).toBe(0);
  });

  test('finds child node', () => {
    const result = findNodeById(nodes, 'child1');
    expect(result).toBeDefined();
    expect(result?.node.content).toBe('Child 1');
    expect(result?.index).toBe(0);
  });

  test('finds deeply nested node', () => {
    const result = findNodeById(nodes, 'grandchild');
    expect(result).toBeDefined();
    expect(result?.node.content).toBe('Grandchild');
  });

  test('returns undefined for non-existent id', () => {
    const result = findNodeById(nodes, 'nonexistent');
    expect(result).toBeUndefined();
  });
});

describe('findParentNode', () => {
  const nodes: OutlinerNode[] = [
    {
      id: 'root1',
      content: 'Root 1',
      children: [
        { id: 'child1', content: 'Child 1', children: [] },
        {
          id: 'child2',
          content: 'Child 2',
          children: [
            { id: 'grandchild', content: 'Grandchild', children: [] },
          ],
        },
      ],
    },
  ];

  test('returns undefined for root node', () => {
    const result = findParentNode(nodes, 'root1');
    expect(result).toBeUndefined();
  });

  test('finds parent of child node', () => {
    const result = findParentNode(nodes, 'child1');
    expect(result).toBeDefined();
    expect(result?.id).toBe('root1');
  });

  test('finds parent of grandchild node', () => {
    const result = findParentNode(nodes, 'grandchild');
    expect(result).toBeDefined();
    expect(result?.id).toBe('child2');
  });

  test('returns undefined for non-existent id', () => {
    const result = findParentNode(nodes, 'nonexistent');
    expect(result).toBeUndefined();
  });
});

describe('flattenNodes', () => {
  test('flattens empty array', () => {
    expect(flattenNodes([])).toEqual([]);
  });

  test('flattens single node', () => {
    const nodes: OutlinerNode[] = [
      { id: '1', content: 'Item', children: [] },
    ];
    expect(flattenNodes(nodes)).toHaveLength(1);
  });

  test('flattens in document order', () => {
    const nodes: OutlinerNode[] = [
      {
        id: '1',
        content: 'First',
        children: [
          { id: '2', content: 'Second', children: [] },
          { id: '3', content: 'Third', children: [] },
        ],
      },
      { id: '4', content: 'Fourth', children: [] },
    ];
    const flat = flattenNodes(nodes);
    expect(flat.map(n => n.id)).toEqual(['1', '2', '3', '4']);
  });
});

describe('getPreviousNode and getNextNode', () => {
  const nodes: OutlinerNode[] = [
    {
      id: '1',
      content: 'First',
      children: [
        { id: '2', content: 'Second', children: [] },
      ],
    },
    { id: '3', content: 'Third', children: [] },
  ];

  test('getPreviousNode returns previous in document order', () => {
    expect(getPreviousNode(nodes, '1')).toBeUndefined();
    expect(getPreviousNode(nodes, '2')?.id).toBe('1');
    expect(getPreviousNode(nodes, '3')?.id).toBe('2');
  });

  test('getNextNode returns next in document order', () => {
    expect(getNextNode(nodes, '1')?.id).toBe('2');
    expect(getNextNode(nodes, '2')?.id).toBe('3');
    expect(getNextNode(nodes, '3')).toBeUndefined();
  });
});

describe('createNode', () => {
  test('creates node with empty content', () => {
    const node = createNode();
    expect(node.content).toBe('');
    expect(node.children).toEqual([]);
    expect(node.id).toBeDefined();
  });

  test('creates node with content', () => {
    const node = createNode('Test content');
    expect(node.content).toBe('Test content');
  });

  test('creates nodes with unique IDs', () => {
    const node1 = createNode();
    const node2 = createNode();
    expect(node1.id).not.toBe(node2.id);
  });
});

describe('cloneNodes', () => {
  test('clones empty array', () => {
    expect(cloneNodes([])).toEqual([]);
  });

  test('clones nodes preserving structure', () => {
    const nodes: OutlinerNode[] = [
      {
        id: '1',
        content: 'Parent',
        children: [
          { id: '2', content: 'Child', children: [] },
        ],
      },
    ];
    const cloned = cloneNodes(nodes);
    expect(cloned).not.toBe(nodes);
    expect(cloned[0]).not.toBe(nodes[0]);
    expect(cloned[0].content).toBe('Parent');
    expect(cloned[0].children[0].content).toBe('Child');
  });

  test('keeps original IDs by default', () => {
    const nodes: OutlinerNode[] = [
      { id: 'original-id', content: 'Item', children: [] },
    ];
    const cloned = cloneNodes(nodes);
    expect(cloned[0].id).toBe('original-id');
  });

  test('regenerates IDs when requested', () => {
    const nodes: OutlinerNode[] = [
      { id: 'original-id', content: 'Item', children: [] },
    ];
    const cloned = cloneNodes(nodes, true);
    expect(cloned[0].id).not.toBe('original-id');
    expect(cloned[0].content).toBe('Item');
  });
});
