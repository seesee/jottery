/**
 * WYSIWYG Markdown support
 *
 * The visual editor is Tiptap, and markdown parsing/serialisation is done
 * directly against the editor schema by `@tiptap/markdown`. This module holds
 * the pieces that are specific to jottery:
 *
 * - `AttachmentImage`: images whose markdown source is an `attachment:` URL.
 *   The editor shows a resolved blob URL, but the markdown keeps the reference.
 * - `NoteLink`: links using the `link:` protocol, including the empty-text
 *   form `[](link:uuid)` which ProseMirror cannot represent without a
 *   placeholder character.
 * - `SoftHardBreak`: jottery renders markdown with `breaks: true` everywhere,
 *   so a hard break is written back as a plain newline rather than two
 *   trailing spaces.
 * - `escapeMarkdownText` / `installMarkdownEscaping`: context-aware escaping
 *   of literal text. The library escapes every `_ * [ ] ~` unconditionally,
 *   which rewrites ordinary prose such as `snake_case`; this replaces that
 *   with escaping only where the character would otherwise change meaning.
 */

import type { Editor, JSONContent } from '@tiptap/core';
import { Markdown } from '@tiptap/markdown';
import { Image } from '@tiptap/extension-image';
import { Link } from '@tiptap/extension-link';
import { HardBreak } from '@tiptap/extension-hard-break';
import { ListItem } from '@tiptap/extension-list';
import { StarterKit } from '@tiptap/starter-kit';
import { CodeBlockLowlight } from '@tiptap/extension-code-block-lowlight';
import { Table } from '@tiptap/extension-table';
import { TableRow } from '@tiptap/extension-table-row';
import { TableCell } from '@tiptap/extension-table-cell';
import { TableHeader } from '@tiptap/extension-table-header';
import { TaskList } from '@tiptap/extension-task-list';
import { TaskItem } from '@tiptap/extension-task-item';
import { Placeholder } from '@tiptap/extension-placeholder';
import { Typography } from '@tiptap/extension-typography';
import { common, createLowlight } from 'lowlight';

/** Placeholder text shown for note links with no visible text. */
export const EMPTY_LINK_PLACEHOLDER = '🔗';

const ATTACHMENT_PREFIX = 'attachment:';
const NOTE_LINK_PREFIX = 'link:';

/**
 * Image node that keeps the original `attachment:` reference in a data
 * attribute so it survives the trip through the editor.
 */
export const AttachmentImage = Image.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      'data-attachment-url': {
        default: null,
        parseHTML: (element: HTMLElement) => element.getAttribute('data-attachment-url'),
        renderHTML: (attributes: Record<string, unknown>) => {
          if (!attributes['data-attachment-url']) return {};
          return { 'data-attachment-url': attributes['data-attachment-url'] };
        },
      },
    };
  },

  parseMarkdown: (token, helpers) => {
    const href: string = token.href ?? '';
    const alt: string = token.text ?? '';
    const title: string | null = token.title ?? null;
    if (href.startsWith(ATTACHMENT_PREFIX)) {
      // src is filled in asynchronously by the component once the attachment
      // blob has been decrypted; until then the image renders as a placeholder
      return helpers.createNode('image', {
        src: '',
        alt,
        title,
        'data-attachment-url': href,
      });
    }
    return helpers.createNode('image', { src: href, alt, title });
  },

  renderMarkdown: (node) => {
    const attrs = node.attrs ?? {};
    const src: string = attrs['data-attachment-url'] || attrs.src || '';
    const alt: string = attrs.alt ?? '';
    const title: string = attrs.title ?? '';
    return title ? `![${alt}](${src} "${title}")` : `![${alt}](${src})`;
  },
});

/**
 * Link mark that understands jottery's `link:` note references and writes
 * bare URLs back without wrapping them in `[url](url)`.
 */
export const NoteLink = Link.extend({
  parseMarkdown: (token, helpers) => {
    const href: string = token.href ?? '';
    const title = token.title || null;
    let content = helpers.parseInline(token.tokens || []);
    if (content.length === 0 && href.startsWith(NOTE_LINK_PREFIX)) {
      content = [helpers.createTextNode(EMPTY_LINK_PLACEHOLDER)];
    }
    return helpers.applyMark('link', content, { href, title });
  },

  renderMarkdown: (node, helpers) => {
    const href: string = node.attrs?.href ?? '';
    const title: string = node.attrs?.title ?? '';
    let text = helpers.renderChildren(node);
    if (href.startsWith(NOTE_LINK_PREFIX) && text.trim() === EMPTY_LINK_PLACEHOLDER) {
      text = '';
    }
    if (!title && text === href && /^https?:\/\//.test(href)) {
      return href;
    }
    return title ? `[${text}](${href} "${title}")` : `[${text}](${href})`;
  },
});

/**
 * Hard break rendered as a bare newline. Every markdown renderer in jottery
 * uses `breaks: true`, so a newline already means a line break and the
 * two-trailing-spaces form would only add invisible noise to every line.
 */
export const SoftHardBreak = HardBreak.extend({
  renderMarkdown: () => '\n',
});

type ItemRenderer = (node: JSONContent, helpers: any, context: any) => string;

/**
 * The library indents nested blocks inside a list item but not the
 * continuation lines of the item's first paragraph, so
 * `- item with\n  continuation` lost its indentation. Lazy continuation
 * keeps it valid markdown, but the text still changed; this pads those
 * lines to line up with the item content.
 */
function withContinuationIndent(render: ItemRenderer, alignToPrefix: boolean): ItemRenderer {
  return (node, helpers, context) => {
    const output = render(node, helpers, context);
    const first = node.content?.[0];
    if (!first) return output;
    const firstLines = helpers.renderChildren([first]).split('\n');
    if (firstLines.length < 2) return output;
    const lines = output.split('\n');
    // Task items keep the library's nested indent: marked reads a deeper
    // indent after `- [ ] ` as an indented code block
    const pad = alignToPrefix
      ? ' '.repeat(Math.max(0, lines[0].length - firstLines[0].length))
      : helpers.indent('');
    for (let i = 1; i < firstLines.length && i < lines.length; i++) {
      if (lines[i]) lines[i] = pad + lines[i];
    }
    return lines.join('\n');
  };
}

/** Bullet and numbered list items with indented continuation lines. */
export const JotteryListItem = ListItem.extend({
  renderMarkdown: withContinuationIndent(ListItem.config.renderMarkdown as ItemRenderer, true),
});

/** Task list items with indented continuation lines. */
export const JotteryTaskItem = TaskItem.extend({
  renderMarkdown: withContinuationIndent(TaskItem.config.renderMarkdown as ItemRenderer, false),
});

/** The configured markdown extension shared by the editor and tests. */
export const JotteryMarkdown = Markdown.configure({
  indentation: { style: 'space', size: 2 },
  markedOptions: { gfm: true, breaks: true },
});

const ASCII_PUNCT = /[!-/:-@[-`{-~]/;

function isWordChar(ch: string | undefined): boolean {
  return ch !== undefined && /[\p{L}\p{N}]/u.test(ch);
}

function isSpace(ch: string | undefined): boolean {
  return ch === undefined || /\s/.test(ch);
}

/**
 * Escape a run of literal text so that it parses back to the same text.
 *
 * Only characters that would actually be interpreted as markdown in their
 * position are escaped:
 * - `*` and `~` when adjacent to non-whitespace (where they could delimit
 *   emphasis or strikethrough)
 * - `_` in the same positions, except inside a word where GFM never treats
 *   it as emphasis (`snake_case`)
 * - `` ` `` always (a stray backtick can pair with a later one)
 * - `\` only before ASCII punctuation, where it would itself be an escape
 * - `[`/`]` only when the text also contains a `](` or `][` sequence
 * - block-level markers (`#`, `>`, list bullets, numbered items, rules,
 *   setext underlines) only at the start of a line
 */
export function escapeMarkdownText(text: string, atLineStart: boolean): string {
  const chars = Array.from(text);
  const linkLike = /\]\s*[([]/.test(text);
  let out = '';

  for (let i = 0; i < chars.length; i++) {
    const ch = chars[i];
    const prev = chars[i - 1];
    const next = chars[i + 1];

    switch (ch) {
      case '\\':
        out += next !== undefined && ASCII_PUNCT.test(next) ? '\\\\' : '\\';
        break;
      case '`':
        out += '\\`';
        break;
      case '*':
      case '~':
        out += isSpace(prev) && isSpace(next) ? ch : `\\${ch}`;
        break;
      case '_':
        if ((isWordChar(prev) && isWordChar(next)) || (isSpace(prev) && isSpace(next))) {
          out += ch;
        } else {
          out += '\\_';
        }
        break;
      case '[':
      case ']':
        out += linkLike ? `\\${ch}` : ch;
        break;
      default:
        out += ch;
    }
  }

  if (atLineStart) {
    out = out
      // headings, blockquotes
      .replace(/^(\s*)(#{1,6})(?=\s|$)/, '$1\\$2')
      .replace(/^(\s*)>/, '$1\\>')
      // bullet list markers and thematic breaks (`*` and `_` are already escaped)
      .replace(/^(\s*)([-+])(?=\s|$)/, '$1\\$2')
      .replace(/^(\s*)(-{3,}|={3,})\s*$/, '$1\\$2')
      // ordered list markers
      .replace(/^(\s*\d{1,9})([.)])(?=\s|$)/, '$1\\$2')
      // task list markers
      .replace(/^(\s*)\[(?=[ xX]\])/, '$1\\[');
  }

  return out;
}

/** Minimal view of the manager internals this module overrides. */
interface EscapingManager {
  codeTypes: Set<string>;
  encodeTextForMarkdown(text: string, node: JSONContent, parentNode?: JSONContent): string;
  renderNodeToMarkdown(node: JSONContent, ...rest: unknown[]): string;
}

function isInsideCode(manager: EscapingManager, node: JSONContent, parentNode?: JSONContent): boolean {
  if (parentNode?.type && manager.codeTypes.has(parentNode.type)) return true;
  return (node.marks ?? []).some((mark) => manager.codeTypes.has(typeof mark === 'string' ? mark : mark.type));
}

/**
 * Replace the markdown manager's unconditional escaping with
 * `escapeMarkdownText`. Call once, immediately after creating the editor.
 */
export function installMarkdownEscaping(editor: Editor): void {
  const manager = (editor.storage as { markdown?: { manager?: unknown } }).markdown?.manager as
    | EscapingManager
    | undefined;
  if (!manager) return;

  // Track whether we are inside a table: a literal `|` there must be
  // escaped (even inside code spans) or it splits the cell on re-parse
  let tableDepth = 0;
  const renderNode = manager.renderNodeToMarkdown.bind(manager);
  manager.renderNodeToMarkdown = (node, ...rest) => {
    if (node.type !== 'table') return renderNode(node, ...rest);
    tableDepth++;
    try {
      return renderNode(node, ...rest);
    } finally {
      tableDepth--;
    }
  };

  manager.encodeTextForMarkdown = (text, node, parentNode) => {
    const escapePipes = (value: string) => (tableDepth > 0 ? value.replace(/\|/g, '\\|') : value);
    if (isInsideCode(manager, node, parentNode)) return escapePipes(text);
    const siblings = parentNode?.content ?? [];
    const index = siblings.indexOf(node);
    const previous = index > 0 ? siblings[index - 1] : undefined;
    const afterBreak = previous?.type === 'hardBreak';
    // marked keeps any indentation beyond the list marker on continuation
    // lines; it is insignificant and would otherwise grow on every round trip
    const cleaned = afterBreak ? text.replace(/^[ \t]+/, '') : text;
    return escapePipes(escapeMarkdownText(cleaned, index <= 0 || afterBreak));
  };
}

const FENCE = /^\s*(```|~~~)/;

/**
 * Tidy the serialiser output:
 * - marks are rendered from a synthetic placeholder node, so a link's text is
 *   not available to `renderMarkdown`; empty note links and bare URLs are
 *   therefore rewritten here instead
 * - the parser appends an empty paragraph after a trailing block so there is
 *   somewhere to type, which would otherwise serialise as blank lines
 */
export function postProcessMarkdown(markdown: string): string {
  let inFence = false;
  const lines = markdown.split('\n').map((line) => {
    if (FENCE.test(line)) {
      inFence = !inFence;
      return line;
    }
    if (inFence) return line;
    return line
      .replace(new RegExp(`\\[${EMPTY_LINK_PLACEHOLDER}\\]\\((${NOTE_LINK_PREFIX}[^)\\s]+)\\)`, 'g'), '[]($1)')
      .replace(/\[(https?:\/\/[^\]\s]+)\]\(\1\)/g, '$1');
  });
  return lines.join('\n').replace(/^\n+/, '').replace(/\s+$/, '');
}

/** Serialise the editor document to markdown. Use instead of `editor.getMarkdown()`. */
export function getMarkdownFromEditor(editor: Editor): string {
  return postProcessMarkdown(editor.getMarkdown());
}

export interface WysiwygExtensionOptions {
  /** Placeholder shown in an empty editor. */
  placeholder?: string;
  /** Extra classes applied to link elements. */
  linkClass?: string;
  /** Extra classes applied to image elements. */
  imageClass?: string;
}

/**
 * The complete extension set for the visual editor. Shared with the
 * round-trip tests so they exercise exactly what the component runs.
 */
export function createWysiwygExtensions(options: WysiwygExtensionOptions = {}) {
  const lowlight = createLowlight(common);
  return [
    StarterKit.configure({
      codeBlock: false, // CodeBlockLowlight below
      hardBreak: false, // SoftHardBreak below
      link: false, // NoteLink below
      listItem: false, // JotteryListItem below
    }),
    JotteryListItem,
    CodeBlockLowlight.configure({ lowlight }),
    NoteLink.configure({
      openOnClick: false,
      HTMLAttributes: options.linkClass ? { class: options.linkClass } : {},
      protocols: ['http', 'https', 'mailto', 'tel', 'link'],
      validate: (href: string) => /^(https?:\/\/|mailto:|tel:|link:)/.test(href),
    }),
    AttachmentImage.configure({
      HTMLAttributes: options.imageClass ? { class: options.imageClass } : {},
    }),
    Table.configure({ resizable: true }),
    TableRow,
    TableCell,
    TableHeader,
    TaskList,
    JotteryTaskItem.configure({ nested: true }),
    Placeholder.configure({ placeholder: options.placeholder ?? '' }),
    Typography,
    SoftHardBreak,
    JotteryMarkdown,
  ];
}
