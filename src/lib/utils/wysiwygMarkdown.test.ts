import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { Editor } from '@tiptap/core';
import {
  createWysiwygExtensions,
  installMarkdownEscaping,
  getMarkdownFromEditor,
  escapeMarkdownText,
  EMPTY_LINK_PLACEHOLDER,
} from './wysiwygMarkdown';

let editor: Editor;

beforeAll(() => {
  editor = new Editor({ extensions: createWysiwygExtensions(), content: '' });
  installMarkdownEscaping(editor);
});

afterAll(() => {
  editor.destroy();
});

/** markdown -> editor -> markdown, as happens on the first keystroke in Visual mode */
function roundtrip(md: string): string {
  editor.commands.setContent(md, { contentType: 'markdown' });
  return getMarkdownFromEditor(editor);
}

/** Cases where the markdown should survive the round trip byte for byte. */
const identical: Record<string, string> = {
  'intra-word underscores and spaced asterisks':
    'Use snake_case_name and 2 * 3 * 4 = 24, also a_b_c',
  'windows paths, prices, brackets, hashes': 'Path C:\\Users\\chris\\file.txt, price $5, #notaheading, [brackets] and (parens), a + b - c',
  'bold, italic, strike, code': 'Some **bold**, *em*, ***both***, ~~strike~~, `code with` + `backtick`',
  'single newlines within a paragraph': 'line one\nline two\nline three\n\nnew paragraph',
  'atx headings': '# H1\n\n## H2\n\n### H3 with `code` and *em*',
  'nested bullet and numbered lists':
    '- one\n- two\n  - two a\n  - two b\n    - two b i\n- three\n\n1. first\n2. second\n   1. nested\n3. third',
  'ordered list starting at 3': '3. three\n4. four\n5. five',
  'list item with a continuation line': '- item with\n  continuation line\n- next',
  'numbered item with a continuation line': '1. item with\n   continuation line\n2. next',
  'pipe inside a table cell code span': '| a        | b   |\n| -------- | --- |\n| `x \\| y` | z   |',
  'fenced code block with blank lines': '```python\ndef f():\n    return "hi"\n\n\nprint(f())\n```\n\ntext after',
  'fenced code block without language': '```\nplain <b>not bold</b> & stuff *and* _this_\n```',
  'blockquote with paragraphs': '> quoted line one\n> quoted line two\n>\n> second para',
  'table with inline formatting': '| Name     | Value               |\n| -------- | ------------------- |\n| **bold** | `code`              |\n| a \\| b   | [link](https://x.y) |',
  'task list': '- [ ] todo\n- [x] done\n  - [ ] nested todo',
  'links with title, bare url and image': '[text](https://example.com "Title") and https://bare.link\n\n![alt](https://img/x.png "img title")',
  'note links and attachments': 'See [other note](link:abc-123) and [](link:def-456)\n\n![shot](attachment:att-789)\n\n![](attachment:att-000 "titled")',
  'horizontal rule': 'above\n\n---\n\nbelow',
  'inline math-ish text': 'Inline $x^2 + y_1$ and 🎉 emoji and – en dash — em dash … ellipsis',
  'typography characters': 'Ranges 1--2, arrows -> <-, (c) 2026, 1/2 fraction, "smart" quotes...',
  'heading directly followed by list': '## Title\n\n- a\n- b\n\nText',
  'entities and comparison operators': 'AT&T, 5 < 6 > 4, "quotes" and \'single\'',
  'literal hash and bullet at line start inside a paragraph': 'Note:\n\\# not a heading\n\\- not a bullet',
  'unmatched emphasis characters': 'file\\*.txt and \\_private and trailing\\_',
};

/** Cases where the round trip normalises syntax but keeps meaning. */
const normalised: Record<string, [string, string]> = {
  'two-space and backslash hard breaks become plain newlines': ['two-space  \nnext\\\nlast', 'two-space\nnext\nlast'],
  'closed atx and setext headings become plain atx': ['## H2 ##\n\nSetext\n======', '## H2\n\n# Setext'],
  'underscore emphasis becomes asterisk emphasis': ['__strong__ and _em_', '**strong** and *em*'],
  'indented code becomes fenced': ['para\n\n    indented\n    more\n\nafter', 'para\n\n```\nindented\nmore\n```\n\nafter'],
  'angle-bracket autolink becomes bare url': ['<https://auto.link>', 'https://auto.link'],
  'loose list becomes tight': ['- a\n\n- b', '- a\n- b'],
  'table columns are padded': ['| a | b |\n| --- | --- |\n| longer | x |', '| a      | b   |\n| ------ | --- |\n| longer | x   |'],
  'trailing blank lines are dropped': ['text\n\n\n', 'text'],
  // the task-list tokeniser treats an indented continuation as nested content
  'task item continuation line becomes a second paragraph': ['- [ ] task with\n  continuation line', '- [ ] task with\n\n  continuation line'],
};

describe('wysiwyg markdown round trip', () => {
  describe('is lossless for', () => {
    for (const [name, md] of Object.entries(identical)) {
      it(name, () => {
        expect(roundtrip(md)).toBe(md);
      });
    }
  });

  describe('normalises but preserves meaning for', () => {
    for (const [name, [md, expected]] of Object.entries(normalised)) {
      it(name, () => {
        expect(roundtrip(md)).toBe(expected);
      });
    }
  });

  it('is idempotent for every case', () => {
    const all = [...Object.values(identical), ...Object.values(normalised).map(([md]) => md)];
    for (const md of all) {
      const once = roundtrip(md);
      expect(roundtrip(once)).toBe(once);
    }
  });

  it('shows a placeholder for empty note links inside the editor', () => {
    editor.commands.setContent('[](link:abc)', { contentType: 'markdown' });
    expect(editor.getText()).toBe(EMPTY_LINK_PLACEHOLDER);
    expect(editor.getHTML()).toContain('href="link:abc"');
  });

  it('keeps the attachment reference on the image node and leaves src empty for the component to resolve', () => {
    editor.commands.setContent('![shot](attachment:att-1)', { contentType: 'markdown' });
    const img = editor.getJSON().content?.[0];
    expect(img?.type).toBe('image');
    expect(img?.attrs?.['data-attachment-url']).toBe('attachment:att-1');
    expect(img?.attrs?.src).toBe('');
    expect(editor.getHTML()).toContain('data-attachment-url="attachment:att-1"');
  });

  it('writes the attachment reference even after the component sets a blob src', () => {
    editor.commands.setContent('![shot](attachment:att-1)', { contentType: 'markdown' });
    editor.commands.updateAttributes('image', { src: 'blob:http://localhost/xyz' });
    expect(getMarkdownFromEditor(editor)).toBe('![shot](attachment:att-1)');
  });

  it('returns an empty string for an empty document', () => {
    expect(roundtrip('')).toBe('');
  });
});

describe('escapeMarkdownText', () => {
  it('leaves prose alone', () => {
    expect(escapeMarkdownText('snake_case, 2 * 3, C:\\Users, a [b] c, 50% off!', false)).toBe(
      'snake_case, 2 * 3, C:\\Users, a [b] c, 50% off!'
    );
  });

  it('escapes emphasis delimiters that touch text', () => {
    expect(escapeMarkdownText('file*.txt _private trailing_ ~tilde~', false)).toBe(
      'file\\*.txt \\_private trailing\\_ \\~tilde\\~'
    );
  });

  it('escapes brackets only when they form a link', () => {
    expect(escapeMarkdownText('see [text](url)', false)).toBe('see \\[text\\](url)');
  });

  it('escapes backslashes only before punctuation', () => {
    expect(escapeMarkdownText('C:\\path and \\* star', false)).toBe('C:\\path and \\\\\\* star');
  });

  it('escapes block markers only at the start of a line', () => {
    expect(escapeMarkdownText('# heading', true)).toBe('\\# heading');
    expect(escapeMarkdownText('# heading', false)).toBe('# heading');
    expect(escapeMarkdownText('- bullet', true)).toBe('\\- bullet');
    expect(escapeMarkdownText('1. item', true)).toBe('1\\. item');
    expect(escapeMarkdownText('> quote', true)).toBe('\\> quote');
    expect(escapeMarkdownText('---', true)).toBe('\\---');
    expect(escapeMarkdownText('[ ] task', true)).toBe('\\[ ] task');
    expect(escapeMarkdownText('#notaheading', true)).toBe('#notaheading');
    expect(escapeMarkdownText('a - b', true)).toBe('a - b');
  });

  it('always escapes backticks', () => {
    expect(escapeMarkdownText('a ` b', false)).toBe('a \\` b');
  });
});
