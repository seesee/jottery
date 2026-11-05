/**
 * Allowlist HTML sanitiser, and the escaping helpers for building HTML strings.
 *
 * Note content is written by the user, but it can also arrive from an import, an
 * inbox item, or a sync server that controls the plaintext fields around the
 * ciphertext. Anything rendered as live HTML in the app's origin goes through
 * sanitiseHtml as the last step, over the final string.
 *
 * DOMPurify does the parsing and the walking, because that is where the hard
 * problems are: mutation XSS, DOM clobbering, namespace confusion. The lists here
 * only narrow what it allows, and it runs on a private instance so that nothing
 * else in the page can add hooks to this one, or inherit ours.
 *
 * - Everything is allowed by name. An element or attribute nobody thought about
 *   is removed, not kept.
 * - Foreign content (svg, math) and raw-text elements (style, noscript, textarea
 *   and friends) never survive. Those are what let a string parse differently
 *   the second time, so without them attribute values stay attribute values.
 */

import DOMPurify from 'dompurify';

const XHTML_NAMESPACE = 'http://www.w3.org/1999/xhtml';

/** Removed along with everything inside them. */
const DROP_WITH_CONTENT = new Set([
  'script', 'style', 'template', 'noscript', 'noembed', 'noframes', 'plaintext', 'xmp', 'listing',
  'iframe', 'frame', 'frameset', 'object', 'embed', 'applet', 'portal', 'fencedframe',
  'svg', 'math', 'title', 'textarea', 'select', 'option', 'optgroup', 'datalist',
  'form', 'button', 'meta', 'link', 'base', 'head', 'audio', 'video', 'source', 'track',
  'canvas', 'dialog', 'marquee',
]);

/** Everything else not listed here is unwrapped: its children stay, the tag goes. */
const ALLOWED_TAGS = new Set([
  'a', 'abbr', 'b', 'bdi', 'bdo', 'blockquote', 'br', 'caption', 'cite', 'code', 'col', 'colgroup',
  'dd', 'del', 'details', 'dfn', 'div', 'dl', 'dt', 'em', 'figcaption', 'figure',
  'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'hr', 'i', 'img', 'input', 'ins', 'kbd', 'li', 'mark',
  'ol', 'p', 'pre', 'q', 'rp', 'rt', 'ruby', 's', 'samp', 'small', 'span', 'strike', 'strong',
  'sub', 'summary', 'sup', 'table', 'tbody', 'td', 'tfoot', 'th', 'thead', 'time', 'tr', 'u',
  'ul', 'var', 'wbr',
]);

const GLOBAL_ATTRIBUTES = new Set([
  'class', 'title', 'lang', 'dir',
  // Read back by the preview to resolve attachments and note links. They only
  // ever select from the open note's own attachments, so they grant nothing.
  'data-attachment-id', 'data-attachment-data', 'data-attachment-filename', 'data-loaded', 'data-note-id',
]);

const TAG_ATTRIBUTES: Record<string, Set<string>> = {
  a: new Set(['href', 'target', 'rel', 'id']),
  img: new Set(['src', 'alt', 'width', 'height']),
  input: new Set(['type', 'checked', 'disabled']),
  ol: new Set(['start', 'reversed', 'type']),
  li: new Set(['value']),
  td: new Set(['colspan', 'rowspan', 'align']),
  th: new Set(['colspan', 'rowspan', 'align', 'scope']),
  col: new Set(['span']),
  colgroup: new Set(['span']),
  details: new Set(['open']),
  time: new Set(['datetime']),
  h1: new Set(['id']),
  h2: new Set(['id']),
  h3: new Set(['id']),
  h4: new Set(['id']),
  h5: new Set(['id']),
  h6: new Set(['id']),
};

const LINK_PROTOCOLS = new Set([
  'http:', 'https:', 'ftp:', 'ftps:', 'mailto:', 'tel:', 'callto:', 'sms:', 'xmpp:', 'matrix:',
  // The app's own schemes, handled by the preview's click handler
  'note:', 'link:',
]);

const IMAGE_PROTOCOLS = new Set(['http:', 'https:', 'blob:']);

/** Ids become named properties on window, so keep them to the shape headings produce. */
const SAFE_ID = /^[\w-]{1,200}$/;

/**
 * The scheme a URL really uses. The URL parser strips the tabs, newlines and
 * leading control characters that browsers ignore, so `java\tscript:` can't get
 * past it the way it gets past a prefix check.
 */
function protocolOf(value: string): string | null {
  try {
    return new URL(value, 'https://jottery.invalid/').protocol;
  } catch {
    return null;
  }
}

function isSafeHref(value: string): boolean {
  const protocol = protocolOf(value);
  return protocol !== null && LINK_PROTOCOLS.has(protocol);
}

function isSafeImageSource(value: string): boolean {
  const protocol = protocolOf(value);
  if (protocol === 'data:') {
    return /^\s*data:image\//i.test(value);
  }
  return protocol !== null && IMAGE_PROTOCOLS.has(protocol);
}

function isAttributeAllowed(tag: string, name: string): boolean {
  return GLOBAL_ATTRIBUTES.has(name) || (TAG_ATTRIBUTES[tag]?.has(name) ?? false);
}

function isAttributeValueSafe(tag: string, name: string, value: string): boolean {
  if (name === 'href') return isSafeHref(value);
  if (name === 'src') return isSafeImageSource(value);
  if (name === 'id') return SAFE_ID.test(value);
  if (name === 'target') return value === '_blank';
  return !(tag === 'input' && name === 'type' && value.toLowerCase() !== 'checkbox');
}

/** DOMPurify only needs to check URLs in the attributes that hold them. */
const URL_ATTRIBUTES = new Set(['href', 'src']);

const ALLOWED_ATTRIBUTES = [...GLOBAL_ATTRIBUTES, ...Object.values(TAG_ATTRIBUTES).flatMap(names => [...names])];

/**
 * Its own scheme check, run as well as isSafeHref and isSafeImageSource. The
 * tail is DOMPurify's: anything that isn't a scheme at all, so relative URLs.
 */
const ALLOWED_URI = new RegExp(
  `^(?:(?:${[...LINK_PROTOCOLS, ...IMAGE_PROTOCOLS].map(protocol => protocol.slice(0, -1)).join('|')}):`
  + '|data:image/|[^a-z]|[a-z+.\\-]+(?:[^a-z+.\\-:]|$))',
  'i',
);

const PURIFY_CONFIG = {
  ALLOWED_TAGS: [...ALLOWED_TAGS],
  ALLOWED_ATTR: ALLOWED_ATTRIBUTES,
  ALLOWED_NAMESPACES: [XHTML_NAMESPACE],
  ALLOWED_URI_REGEXP: ALLOWED_URI,
  FORBID_CONTENTS: [...DROP_WITH_CONTENT],
  ADD_URI_SAFE_ATTR: ALLOWED_ATTRIBUTES.filter(name => !URL_ATTRIBUTES.has(name)),
  // On by default in DOMPurify, which would widen the lists above
  ALLOW_DATA_ATTR: false,
  ALLOW_ARIA_ATTR: false,
  // This drops any id or name that is also a property of document, which covers
  // headings such as `# Title` and `# Links` and breaks their anchors. It isn't
  // needed here: only a name can clobber document, and name is never allowed.
  SANITIZE_DOM: false,
};

function createPurifier() {
  // Calling it with a window gives a new instance with its own hooks
  const purifier = DOMPurify(globalThis.window);

  purifier.addHook('uponSanitizeElement', (node, data) => {
    if (data.tagName === 'input' && ((node as Element).getAttribute('type') ?? '').toLowerCase() !== 'checkbox') {
      node.parentNode?.removeChild(node);
    }
  });

  purifier.addHook('uponSanitizeAttribute', (node, data) => {
    const tag = node.nodeName.toLowerCase();
    if (!isAttributeAllowed(tag, data.attrName) || !isAttributeValueSafe(tag, data.attrName, data.attrValue)) {
      data.keepAttr = false;
    }
  });

  purifier.addHook('afterSanitizeAttributes', node => {
    if (node.nodeType !== 1) return;
    const element = node as Element;
    const tag = element.nodeName.toLowerCase();

    if (tag === 'a' && element.hasAttribute('target')) {
      element.setAttribute('rel', 'noopener noreferrer');
    }

    if (tag === 'input') {
      // Only task list checkboxes survive, and a preview can't change anything
      element.setAttribute('disabled', '');
    }
  });

  return purifier;
}

const purifier = createPurifier();

/**
 * Reduce an HTML string to the allowed subset. This must be the final step before
 * the string reaches {@html} or any other sink: running it earlier would miss
 * whatever is interpolated afterwards.
 */
export function sanitiseHtml(html: string): string {
  // Without a DOM, DOMPurify hands back its input. Fail closed instead.
  if (!purifier.isSupported) {
    return escapeHtml(html);
  }
  return purifier.sanitize(html, PURIFY_CONFIG);
}

/** Escape text for an HTML text position. */
export function escapeHtml(value: unknown): string {
  return String(value)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}

/** Escape text for a quoted HTML attribute value. Safe in either kind of quotes. */
export function escapeAttribute(value: unknown): string {
  return escapeHtml(value)
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}
