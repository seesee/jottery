/**
 * Makes happy-dom standard enough to run DOMPurify, which it can't do by itself:
 *
 * - In a browser, nodeName is one accessor on Node.prototype. In happy-dom each
 *   subclass overrides it and the base returns ''. DOMPurify reads the base, for
 *   safety against clobbering, so every tag name comes back empty.
 * - happy-dom's NodeIterator has no removal handling, so a walk ends at the first
 *   node removed, and everything after it is never looked at.
 *
 * Either one means a sanitiser that quietly passes hostile markup, in tests only.
 * happy-dom's parser isn't a browser's either, so a pass here says the lists are
 * right, not that the result is safe. e2e/preview-security.spec.ts runs the same
 * markup through the same code in real Chromium, and that is the one that counts.
 */

function dispatchNodeNameToSubclasses(): void {
  const proto = window.Node.prototype;
  let owner: object | null = proto;
  while (owner && !Object.getOwnPropertyDescriptor(owner, 'nodeName')) {
    owner = Object.getPrototypeOf(owner);
  }
  const base = owner && Object.getOwnPropertyDescriptor(owner, 'nodeName');
  if (!base?.get) return;
  const baseGetter = base.get;

  Object.defineProperty(proto, 'nodeName', {
    configurable: true,
    get(this: Node) {
      for (let p = Object.getPrototypeOf(this); p && p !== proto && p !== owner; p = Object.getPrototypeOf(p)) {
        const getter = Object.getOwnPropertyDescriptor(p, 'nodeName')?.get;
        if (getter) return getter.call(this);
      }
      return baseGetter.call(this);
    },
  });
}

/** Whether a NodeFilter whatToShow mask includes a node. */
function isShown(node: Node, whatToShow: number): boolean {
  return (whatToShow & (1 << (node.nodeType - 1))) !== 0;
}

/**
 * An iterator that survives changes to the tree: each step walks from the root
 * to the first node it hasn't returned. That visits at least what a browser's
 * would, and the quadratic cost doesn't matter at the size of a test.
 */
function createNodeIterator(root: Node, whatToShow = 0xffffffff): Pick<NodeIterator, 'nextNode'> {
  const returned = new Set<Node>();

  return {
    nextNode() {
      const pending: Node[] = [root];
      while (pending.length > 0) {
        const node = pending.pop() as Node;
        if (!returned.has(node) && isShown(node, whatToShow)) {
          returned.add(node);
          return node;
        }
        const children = Array.from(node.childNodes);
        for (let i = children.length - 1; i >= 0; i--) {
          pending.push(children[i]);
        }
      }
      return null;
    },
  };
}

function replaceNodeIterator(): void {
  let owner: object | null = document;
  while (owner && !Object.getOwnPropertyDescriptor(owner, 'createNodeIterator')) {
    owner = Object.getPrototypeOf(owner);
  }
  if (owner) {
    Object.defineProperty(owner, 'createNodeIterator', {
      configurable: true,
      writable: true,
      value: function (root: Node, whatToShow?: number) {
        return createNodeIterator(root, whatToShow);
      },
    });
  }
}

if (typeof window !== 'undefined' && typeof document !== 'undefined') {
  dispatchNodeNameToSubclasses();
  replaceNodeIterator();
}
