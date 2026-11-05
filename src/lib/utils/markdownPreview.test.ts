/**
 * The preview is the one place note and sync data becomes live HTML. Attachment
 * ids and data are plaintext sync fields, so a sync server can put anything in
 * them without holding the key. These tests parse what the renderer returns and
 * look at the resulting DOM, because that is what an attacker needs to reach.
 */
import { describe, it, expect } from 'vitest';
import { renderMarkdown, renderHtml, getPreviewHtml, buildSandboxedPreview } from './markdownPreview';
import type { Attachment } from '../types';

const ID = '3f2b8c1e-7d4a-4e9b-8a6f-1c2d3e4f5a6b';
const OTHER_ID = '9a8b7c6d-5e4f-4a3b-9c2d-1e0f9a8b7c6d';

const PAYLOADS = [
  '"><img src=x onerror="window.__pwned=1">',
  `'><img src=x onerror='window.__pwned=1'>`,
  '" onmouseover="window.__pwned=1" x="',
  '"><script>window.__pwned=1</script>',
  '"><svg onload="window.__pwned=1">',
  '"><iframe srcdoc="<script>parent.__pwned=1</script>"></iframe>',
];

function attachment(overrides: Partial<Attachment> = {}): Attachment {
  return { id: ID, filename: 'x', mimeType: 'image/png', size: 2048, data: ID, ...overrides };
}

function render(content: string, attachments: Attachment[] = [], notes: { id: string; content: string }[] = []) {
  return renderMarkdown(content, { attachments, openLinksInNewTab: true, notes });
}

/** The scheme the browser would act on, after it strips tabs and control characters. */
function protocolOf(value: string): string {
  try {
    return new URL(value, 'https://test.invalid/').protocol;
  } catch {
    return '';
  }
}

/**
 * Parse as the browser would, then report anything that could run.
 *
 * noscript and template count as live: DOMParser runs with scripting off, so it
 * reads noscript content as markup where a real page reads it as raw text. That
 * difference is the classic mutation route, so neither may survive at all.
 */
function live(html: string): string[] {
  const doc = new DOMParser().parseFromString(`<body>${html}</body>`, 'text/html');
  const found: string[] = [];
  const forbidden = ['script', 'iframe', 'object', 'embed', 'svg', 'math', 'style', 'link', 'meta', 'base', 'form', 'noscript', 'template'];
  for (const el of Array.from(doc.body.querySelectorAll('*'))) {
    const tag = el.tagName.toLowerCase();
    if (forbidden.includes(tag)) {
      found.push(`<${tag}>`);
    }
    for (const attr of Array.from(el.attributes)) {
      const name = attr.name.toLowerCase();
      if (name.startsWith('on') || name === 'srcdoc' || name === 'style') {
        found.push(`${tag}[${name}]`);
      }
      if (['href', 'src', 'action', 'formaction', 'xlink:href'].includes(name)) {
        const protocol = protocolOf(attr.value);
        const isImageData = tag === 'img' && /^data:image\//i.test(attr.value);
        if (['javascript:', 'vbscript:', 'data:'].includes(protocol) && !isImageData) {
          found.push(`${tag}[${name}=${attr.value}]`);
        }
      }
    }
  }
  return found;
}

/** The attacker's own markup must not survive as elements. */
function injectedImages(html: string): number {
  const doc = new DOMParser().parseFromString(`<body>${html}</body>`, 'text/html');
  return Array.from(doc.body.querySelectorAll('img')).filter(img => img.getAttribute('src') === 'x').length;
}

describe('renderMarkdown: hostile attachment metadata', () => {
  for (const payload of PAYLOADS) {
    it(`keeps attachment.data inert in an image: ${payload}`, () => {
      const html = render(`![pic](attachment:${ID})`, [attachment({ data: payload })]);
      expect(live(html)).toEqual([]);
      expect(injectedImages(html)).toBe(0);
    });

    it(`keeps attachment.data inert in a download link: ${payload}`, () => {
      const html = render(`[file](attachment:${ID})`, [attachment({ data: payload, mimeType: 'application/pdf' })]);
      expect(live(html)).toEqual([]);
      expect(injectedImages(html)).toBe(0);
    });

    it(`keeps attachment.id inert in a download link: ${payload}`, () => {
      // Matched through the data field, so the hostile id reaches the template
      const html = render(`[file](attachment:${OTHER_ID})`, [
        attachment({ id: payload, data: OTHER_ID, mimeType: 'application/pdf' }),
      ]);
      expect(live(html)).toEqual([]);
      expect(injectedImages(html)).toBe(0);
    });

    it(`keeps a hostile note id inert in a wiki link: ${payload}`, () => {
      const html = render('[[Target]]', [], [{ id: payload, content: 'Target' }]);
      expect(live(html)).toEqual([]);
      expect(injectedImages(html)).toBe(0);
    });
  }

  it('survives a non-numeric size without emitting it', () => {
    const size = '<img src=x onerror=alert(1)>' as unknown as number;
    const html = render(`[file](attachment:${ID})`, [attachment({ size, mimeType: 'application/pdf' })]);
    expect(live(html)).toEqual([]);
    expect(injectedImages(html)).toBe(0);
  });

  it('keeps the real attachment reference intact for the resolver', () => {
    const html = render(`![pic](attachment:${ID})`, [attachment()]);
    const doc = new DOMParser().parseFromString(`<body>${html}</body>`, 'text/html');
    const img = doc.body.querySelector('img.attachment-image');
    expect(img?.getAttribute('data-attachment-id')).toBe(ID);
    expect(img?.getAttribute('data-loaded')).toBe('false');
    expect(img?.getAttribute('src')).toMatch(/^data:image\/svg\+xml/);
  });

  it('keeps download link attributes intact for the resolver', () => {
    const html = render(`[report](attachment:${ID})`, [attachment({ mimeType: 'application/pdf' })]);
    const doc = new DOMParser().parseFromString(`<body>${html}</body>`, 'text/html');
    const div = doc.body.querySelector('.attachment-download');
    expect(div?.getAttribute('data-attachment-id')).toBe(ID);
    expect(div?.getAttribute('data-attachment-data')).toBe(ID);
    expect(div?.textContent).toContain('report');
    expect(div?.textContent).toContain('2.0 KB');
  });
});

describe('renderMarkdown: hostile note content', () => {
  const cases: Array<[string, string]> = [
    ['raw script', '<script>window.__pwned=1</script>'],
    ['event handler', '<img src=x onerror="window.__pwned=1">'],
    ['javascript link', '[click](javascript:window.__pwned=1)'],
    ['entity-encoded javascript link', '<a href="&#106;avascript:window.__pwned=1">x</a>'],
    ['javascript link with a tab', '<a href="java&#9;script:window.__pwned=1">x</a>'],
    ['javascript link with leading control character', '<a href="&#1;javascript:window.__pwned=1">x</a>'],
    ['data html link', '<a href="data:text/html,<script>1</script>">x</a>'],
    ['svg', '<svg><script>window.__pwned=1</script></svg>'],
    ['math', '<math><mtext><style><img src=x onerror=1></style></mtext></math>'],
    ['iframe', '<iframe src="javascript:window.__pwned=1"></iframe>'],
    ['object', '<object data="javascript:1"></object>'],
    ['embed', '<embed src="javascript:1">'],
    ['form with formaction', '<form><button formaction="javascript:1">x</button></form>'],
    ['form clobbering the sanitiser', '<form><input name="attributes"><input name="localName"><img src=x onerror=1></form>'],
    ['style element', '<style>@import "https://evil.example/x.css";</style>'],
    ['style attribute', '<div style="position:fixed;inset:0">x</div>'],
    ['meta refresh', '<meta http-equiv="refresh" content="0;url=https://evil.example">'],
    ['base element', '<base href="https://evil.example/">'],
    ['link element', '<link rel="stylesheet" href="https://evil.example/x.css">'],
    ['noscript mutation', '<noscript><p title="</noscript><img src=x onerror=1>"></noscript>'],
    ['template', '<template><img src=x onerror=1></template>'],
    ['title in link text', '[<img src=x onerror=1>](https://example.com "t")'],
    ['quote in link title', '[x](https://example.com "\\"onmouseover=\\"1")'],
    ['heading', '# <img src=x onerror=1>'],
    ['broken wiki link', '[[<img src=x onerror=1>]]'],
    ['broken link: protocol', '[x](link:"><img%20src=x%20onerror=1>)'],
    ['image alt and title', '![a"><img src=x onerror=1>](attachment:nope "t\\"><img src=x onerror=1>")'],
    ['nested in an unknown tag', '<custom-tag><img src=x onerror=1></custom-tag>'],
  ];

  for (const [name, source] of cases) {
    it(`neutralises ${name}`, () => {
      expect(live(render(source))).toEqual([]);
    });
  }

  it('escapes an error message rather than rendering it', () => {
    const notString = { replace() { throw new Error('<img src=x onerror=1>'); } } as unknown as string;
    const html = render(notString);
    expect(live(html)).toEqual([]);
  });
});

describe('renderMarkdown: ordinary notes keep working', () => {
  it('keeps headings with their anchors', () => {
    const doc = new DOMParser().parseFromString(render('## Some Title'), 'text/html');
    expect(doc.querySelector('h2')?.getAttribute('id')).toBe('some-title');
  });

  it('renders inline markdown inside headings and link text', () => {
    const doc = new DOMParser().parseFromString(render('# A **bold** word\n\n[some `code`](https://example.com)'), 'text/html');
    expect(doc.querySelector('h1 strong')?.textContent).toBe('bold');
    expect(doc.querySelector('a code')?.textContent).toBe('code');
  });

  it('keeps tables, lists, code and quotes', () => {
    const doc = new DOMParser().parseFromString(
      render('| a | b |\n|---|---|\n| 1 | 2 |\n\n- item\n\n> quote\n\n```\ncode\n```\n'),
      'text/html'
    );
    expect(doc.querySelector('table td')?.textContent).toBe('1');
    expect(doc.querySelector('ul li')?.textContent).toBe('item');
    expect(doc.querySelector('blockquote')).not.toBeNull();
    expect(doc.querySelector('pre code')?.textContent).toContain('code');
  });

  it('keeps task list checkboxes, read-only', () => {
    const doc = new DOMParser().parseFromString(render('- [x] done\n- [ ] open'), 'text/html');
    const boxes = Array.from(doc.querySelectorAll('input'));
    expect(boxes).toHaveLength(2);
    expect(boxes.every(b => b.hasAttribute('disabled') && b.getAttribute('type') === 'checkbox')).toBe(true);
    expect(boxes[0].hasAttribute('checked')).toBe(true);
  });

  it('opens external links in a new tab without an opener', () => {
    const doc = new DOMParser().parseFromString(render('[x](https://example.com/?a=1&b=2)'), 'text/html');
    const a = doc.querySelector('a');
    expect(a?.getAttribute('href')).toBe('https://example.com/?a=1&b=2');
    expect(a?.getAttribute('target')).toBe('_blank');
    expect(a?.getAttribute('rel')).toContain('noopener');
  });

  it('keeps note links clickable', () => {
    const doc = new DOMParser().parseFromString(render('[[Target]]', [], [{ id: ID, content: 'Target' }]), 'text/html');
    const a = doc.querySelector('a.note-link');
    expect(a?.getAttribute('href')).toBe(`note:${ID}`);
    expect(a?.getAttribute('data-note-id')).toBe(ID);
    expect(a?.textContent).toContain('Target');
  });

  it('keeps fragment and mail links', () => {
    const doc = new DOMParser().parseFromString(render('[a](#section) [b](mailto:a@example.com)'), 'text/html');
    const links = Array.from(doc.querySelectorAll('a'));
    expect(links.map(l => l.getAttribute('href'))).toEqual(['#section', 'mailto:a@example.com']);
  });

  it('keeps harmless inline HTML', () => {
    const doc = new DOMParser().parseFromString(render('<details><summary>More</summary>Hidden <kbd>K</kbd></details>'), 'text/html');
    expect(doc.querySelector('details summary')?.textContent).toBe('More');
    expect(doc.querySelector('kbd')?.textContent).toBe('K');
  });
});

describe('renderHtml and getPreviewHtml', () => {
  it('does not return raw HTML from the html path', () => {
    expect(live(renderHtml('<img src=x onerror=1><script>1</script>'))).toEqual([]);
  });

  it('is safe for every previewable language', () => {
    const options = { attachments: [], openLinksInNewTab: true };
    for (const language of ['markdown', 'html']) {
      expect(live(getPreviewHtml('<img src=x onerror=1>', language, options))).toEqual([]);
    }
  });
});

describe('buildSandboxedPreview', () => {
  function policyOf(html: string): string {
    const meta = new DOMParser().parseFromString(html, 'text/html').querySelector('meta');
    expect(meta?.getAttribute('http-equiv')).toBe('Content-Security-Policy');
    return meta?.getAttribute('content') as string;
  }

  it('starts with a policy, ahead of anything in the note', () => {
    const html = buildSandboxedPreview('<meta http-equiv="refresh" content="0"><p>x</p>');
    expect(html.indexOf('Content-Security-Policy')).toBeLessThan(html.indexOf('refresh'));
    expect(policyOf(html)).toContain("default-src 'none'");
  });

  it('allows no script and no remote requests', () => {
    const policy = policyOf(buildSandboxedPreview(''));
    expect(policy).not.toContain('script-src');
    expect(policy).not.toMatch(/https?:/);
    expect(policy).not.toContain("'self'");
  });
});

describe('wiki links', () => {
  const ID_A = '11111111-1111-4111-8111-111111111111';
  const ID_B = '22222222-2222-4222-8222-222222222222';
  const linkTo = (html: string, id: string) => html.includes(`note:${id}`) || html.includes(id);

  it('finds a note by its custom title', () => {
    const notes = [{ id: ID_A, content: 'Draft', tags: ['t:Project Plan'] }];

    expect(linkTo(render('[[Project Plan]]', [], notes), ID_A)).toBe(true);
    expect(linkTo(render('[[Draft]]', [], notes), ID_A)).toBe(false);
  });

  it('also finds one by the longer title: prefix, as the note list does', () => {
    const notes = [{ id: ID_A, content: 'Draft', tags: ['title:Project Plan'] }];

    expect(linkTo(render('[[Project Plan]]', [], notes), ID_A)).toBe(true);
  });

  it('still finds a note by its first line, without the markdown', () => {
    const notes = [{ id: ID_A, content: '\n\n# **Heading** one\nmore', tags: [] }];

    expect(linkTo(render('[[Heading one]]', [], notes), ID_A)).toBe(true);
  });

  it('goes to the first of two notes with the same title, as before', () => {
    const notes = [{ id: ID_A, content: 'Same', tags: [] }, { id: ID_B, content: 'same', tags: [] }];

    expect(linkTo(render('[[Same]]', [], notes), ID_A)).toBe(true);
    expect(linkTo(render('[[Same]]', [], notes), ID_B)).toBe(false);
  });

  it('reads each note once, however many links there are or renders happen', () => {
    let reads = 0;
    const notes = [ID_A, ID_B].map((id, index) => ({
      id,
      tags: [],
      get content() { reads++; return `Note ${index}`; },
    }));
    const links = '[[Note 0]] [[Note 1]] [[Note 0]] [[Note 1]] [[Missing]]';

    render(links, [], notes);
    render(links, [], notes);

    expect(reads).toBe(notes.length);
  });

  it('sees a change, because a change makes a new list', () => {
    const before = [{ id: ID_A, content: 'Old', tags: [] }];
    render('[[Old]]', [], before);

    const after = [{ id: ID_A, content: 'New', tags: [] }];

    expect(linkTo(render('[[New]]', [], after), ID_A)).toBe(true);
    expect(linkTo(render('[[Old]]', [], after), ID_A)).toBe(false);
  });
});
