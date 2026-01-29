/**
 * Types for the Outliner document type
 * A hierarchical, collapsible tree structure for organising information
 */

/**
 * A single node in the outline tree
 */
export interface OutlinerNode {
  /** Unique identifier for the node (UUID) */
  id: string;
  /** Text content of this node */
  content: string;
  /** Child nodes */
  children: OutlinerNode[];
}

/**
 * State for a node in the editor (runtime only, not persisted)
 */
export interface OutlinerNodeState {
  /** Whether this node is collapsed (children hidden) */
  collapsed: boolean;
  /** Whether this node is currently focused */
  focused: boolean;
}
