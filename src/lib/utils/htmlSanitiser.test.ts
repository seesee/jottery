import { describe, it, expect, vi } from 'vitest';
import { sanitiseHtml, escapeAttribute, escapeHtml } from './htmlSanitiser';
import { FORBIDDEN_ELEMENTS, HOSTILE_HTML } from '../../test/hostileHtml';

function parse(html: string): HTMLElement {
  return new DOMParser().parseFromString(`<body>${html}</body>`, 'text/html').body;
}

describe('sanitiseHtml: elements', () => {
  it('removes script and its text', () => {
    expect(sanitiseHtml('a<script>evil()</script>b')).toBe('ab');
  });

  it('removes comments, which can hide markup from a second parse', () => {
    expect(sanitiseHtml('a<!-- <img src=x onerror=1> -->b')).toBe('ab');
  });

  it('removes foreign content along with its children', () => {
    expect(sanitiseHtml('<svg><a href="https://example.com">x</a></svg>')).toBe('');
    expect(sanitiseHtml('<math><mi>x</mi></math>')).toBe('');
  });

  it('unwraps an unknown element and cleans what it held', () => {
    const body = parse(sanitiseHtml('<custom-tag><b onclick="1">x</b></custom-tag>'));
    expect(body.querySelector('custom-tag')).toBeNull();
    expect(body.querySelector('b')?.textContent).toBe('x');
    expect(body.querySelector('b')?.hasAttribute('onclick')).toBe(false);
  });

  it('unwraps nested unknown elements all the way down', () => {
    const body = parse(sanitiseHtml('<x-a><x-b><x-c><script>1</script><i>y</i></x-c></x-b></x-a>'));
    expect(body.innerHTML).toBe('<i>y</i>');
  });

  it('handles nesting as deep as a browser parser allows', () => {
    // Chromium's parser stops nesting at 512. The walk is iterative, so depth
    // isn't its limit. Going deeper here only overflows happy-dom's serialiser.
    const depth = 500;
    const html = '<div>'.repeat(depth) + '<img src=x onerror=1>' + '</div>'.repeat(depth);
    expect(sanitiseHtml(html)).not.toContain('onerror');
  });

  it('removes form controls and embeds along with what they hold', () => {
    const elements = ['form', 'button', 'select', 'canvas', 'dialog', 'marquee', 'object', 'applet', 'datalist'];
    for (const tag of elements) {
      expect(sanitiseHtml(`a<${tag}>inside</${tag}>b`), tag).toBe('ab');
    }
  });

  it('removes inputs other than checkboxes', () => {
    expect(sanitiseHtml('<input type="text" value="x">')).toBe('');
    expect(sanitiseHtml('<input>')).toBe('');
  });

  it('makes a checkbox read-only', () => {
    expect(parse(sanitiseHtml('<input type="checkbox">')).querySelector('input')?.hasAttribute('disabled')).toBe(true);
  });
});

describe('sanitiseHtml: attributes', () => {
  it('removes every event handler', () => {
    const body = parse(sanitiseHtml('<p onclick="1" ONMOUSEOVER="2" onfoo="3">x</p>'));
    expect(body.querySelector('p')?.attributes.length).toBe(0);
  });

  it('removes style', () => {
    expect(sanitiseHtml('<p style="position:fixed">x</p>')).toBe('<p>x</p>');
  });

  it('removes name, which can shadow document properties', () => {
    expect(parse(sanitiseHtml('<a name="cookie" href="#x">y</a>')).querySelector('a')?.hasAttribute('name')).toBe(false);
  });

  it('keeps an id only on headings and anchors, in a safe shape', () => {
    expect(parse(sanitiseHtml('<h1 id="ok-1">x</h1>')).querySelector('h1')?.id).toBe('ok-1');
    expect(parse(sanitiseHtml('<p id="app">x</p>')).querySelector('p')?.hasAttribute('id')).toBe(false);
    expect(parse(sanitiseHtml('<h1 id="a b">x</h1>')).querySelector('h1')?.hasAttribute('id')).toBe(false);
  });

  it('adds rel to any link that keeps a target', () => {
    const a = parse(sanitiseHtml('<a href="https://example.com" target="_blank" rel="opener">x</a>')).querySelector('a');
    expect(a?.getAttribute('rel')).toBe('noopener noreferrer');
  });

  it('removes a target other than _blank', () => {
    const a = parse(sanitiseHtml('<a href="https://example.com" target="_top">x</a>')).querySelector('a');
    expect(a?.hasAttribute('target')).toBe(false);
  });
});

describe('sanitiseHtml: what the preview depends on', () => {
  it('keeps a heading id that is also the name of a document property', () => {
    // `# Title` renders as id="title", and in-note anchors scroll to it
    for (const id of ['title', 'body', 'images', 'links', 'forms', 'location']) {
      expect(parse(sanitiseHtml(`<h2 id="${id}">x</h2>`)).querySelector('h2')?.id).toBe(id);
    }
  });

  it('keeps data attributes whose values look like URLs', () => {
    const body = parse(sanitiseHtml('<a href="note:x" data-note-id="Plans: 2026">y</a>'));
    expect(body.querySelector('a')?.getAttribute('data-note-id')).toBe('Plans: 2026');
  });

  it('keeps plain attributes whose values contain a colon', () => {
    const body = parse(sanitiseHtml('<time datetime="ten:30">x</time><p title="note: this">y</p><img alt="a: b" src="https://example.com/x.png">'));
    expect(body.querySelector('time')?.getAttribute('datetime')).toBe('ten:30');
    expect(body.querySelector('p')?.getAttribute('title')).toBe('note: this');
    expect(body.querySelector('img')?.getAttribute('alt')).toBe('a: b');
  });

  it('removes data and aria attributes that are not on the list', () => {
    const p = parse(sanitiseHtml('<p data-evil="1" aria-label="x">y</p>')).querySelector('p');
    expect(p?.attributes.length).toBe(0);
  });

  it('removes an attribute allowed on one element from another', () => {
    expect(parse(sanitiseHtml('<p href="https://example.com" src="https://example.com/x.png">x</p>')).querySelector('p')?.attributes.length).toBe(0);
    expect(parse(sanitiseHtml('<div open colspan="2">x</div>')).querySelector('div')?.attributes.length).toBe(0);
  });

  it('keeps the markup syntax highlighting and task lists produce', () => {
    const html = '<pre><code class="hljs language-ts"><span class="hljs-keyword">const</span> x</code></pre>'
      + '<ul><li><input type="checkbox" checked disabled> done</li></ul>';
    const body = parse(sanitiseHtml(html));
    expect(body.querySelector('span.hljs-keyword')?.textContent).toBe('const');
    expect(body.querySelector('input')?.hasAttribute('checked')).toBe(true);
  });

  it('keeps tables whole', () => {
    const body = parse(sanitiseHtml('<table><colgroup><col span="2"></colgroup><thead><tr><th scope="col">a</th></tr></thead><tbody><tr><td colspan="2">b</td></tr></tbody></table>'));
    expect(body.querySelector('thead th')?.textContent).toBe('a');
    expect(body.querySelector('td')?.getAttribute('colspan')).toBe('2');
    expect(body.querySelector('col')?.getAttribute('span')).toBe('2');
  });

  it('leaves escaped markup in code as text', () => {
    expect(sanitiseHtml('<code>&lt;img src=x onerror=1&gt;</code>')).toBe('<code>&lt;img src=x onerror=1&gt;</code>');
  });
});

describe('sanitiseHtml: URLs', () => {
  const hostile = [
    'javascript:alert(1)',
    'JaVaScRiPt:alert(1)',
    ' javascript:alert(1)',
    'java\tscript:alert(1)',
    'java\nscript:alert(1)',
    '\u0001javascript:alert(1)',
    'vbscript:msgbox(1)',
    'data:text/html,<script>1</script>',
    'blob:https://example.com/x',
    'file:///etc/passwd',
    'unknownscheme:x',
  ];

  for (const href of hostile) {
    it(`drops a link to ${JSON.stringify(href)}`, () => {
      const body = parse('<a>x</a>');
      body.querySelector('a')?.setAttribute('href', href);
      expect(parse(sanitiseHtml(body.innerHTML)).querySelector('a')?.hasAttribute('href')).toBe(false);
    });
  }

  const allowed = ['https://example.com', 'http://example.com', 'mailto:a@example.com', 'tel:+441234', '#anchor', 'relative/path', 'note:abc', 'link:abc'];

  for (const href of allowed) {
    it(`keeps a link to ${href}`, () => {
      const body = parse('<a>x</a>');
      body.querySelector('a')?.setAttribute('href', href);
      expect(parse(sanitiseHtml(body.innerHTML)).querySelector('a')?.getAttribute('href')).toBe(href);
    });
  }

  it('keeps raster and svg data images as img sources, which cannot run script', () => {
    const src = 'data:image/png;base64,AAAA';
    expect(parse(sanitiseHtml(`<img src="${src}">`)).querySelector('img')?.getAttribute('src')).toBe(src);
  });

  it('drops a data URL that is not an image', () => {
    expect(parse(sanitiseHtml('<img src="data:text/html,x">')).querySelector('img')?.hasAttribute('src')).toBe(false);
  });

  it('drops a javascript image source', () => {
    expect(parse(sanitiseHtml('<img src="javascript:1">')).querySelector('img')?.hasAttribute('src')).toBe(false);
  });

  it('keeps the blob URLs attachments are shown from, on images only', () => {
    const src = 'blob:https://example.com/abc';
    expect(parse(sanitiseHtml(`<img src="${src}">`)).querySelector('img')?.getAttribute('src')).toBe(src);
  });

  it('drops an image source that is only valid for a link', () => {
    expect(parse(sanitiseHtml('<img src="mailto:a@example.com">')).querySelector('img')?.hasAttribute('src')).toBe(false);
  });

  it('keeps an empty image source, which an unresolved attachment uses', () => {
    expect(parse(sanitiseHtml('<img src="" data-attachment-id="x">')).querySelector('img')?.getAttribute('data-attachment-id')).toBe('x');
  });
});

describe('sanitiseHtml: stability', () => {
  const samples = [
    '<p title="&quot;&gt;&lt;img src=x onerror=1&gt;">x</p>',
    '<table><tr><td>a<img src=x onerror=1></td></tr></table>',
    '<a href="https://example.com"><a href="https://example.org">nested</a></a>',
    '<noscript><p title="</noscript><img src=x onerror=1>"></noscript>',
    '<svg></p><style><a id="</style><img src=1 onerror=alert(1)>">',
    '<math><mtext><table><mglyph><style><img src=1 onerror=alert(1)>',
    '<select><template><style><!--</style><a rel="--><img src=x onerror=1>">',
    '<form><math><mtext></form><form><mglyph><style></math><img src=x onerror=1>',
  ];

  for (const sample of samples) {
    it(`does not change when sanitised again: ${sample}`, () => {
      const once = sanitiseHtml(sample);
      expect(sanitiseHtml(once)).toBe(once);
      expect(Array.from(parse(once).querySelectorAll('*')).some(el =>
        Array.from(el.attributes).some(a => a.name.startsWith('on'))
      )).toBe(false);
    });
  }
});

describe('sanitiseHtml: hostile markup', () => {
  for (const sample of HOSTILE_HTML) {
    it(`leaves nothing active in ${sample}`, () => {
      const body = parse(sanitiseHtml(sample));
      expect(body.querySelector(FORBIDDEN_ELEMENTS)).toBeNull();
      for (const element of Array.from(body.querySelectorAll('*'))) {
        expect(Array.from(element.attributes).filter(a => a.name.startsWith('on'))).toEqual([]);
        expect(element.getAttribute('href') ?? '').not.toMatch(/^\s*(javascript|data):/i);
        expect(element.getAttribute('src') ?? '').not.toMatch(/^\s*(javascript|data:text)/i);
      }
    });
  }
});

describe('sanitiseHtml: without a usable DOM', () => {
  it('escapes everything rather than handing the input back', async () => {
    // DOMPurify returns its input untouched when it can't run. Failing open is
    // the one thing a sanitiser mustn't do.
    vi.resetModules();
    vi.doMock('dompurify', () => ({
      default: () => ({ isSupported: false, addHook: () => undefined, sanitize: (dirty: string) => dirty }),
    }));
    try {
      const { sanitiseHtml: unsupported } = await import('./htmlSanitiser');
      expect(unsupported('<img src=x onerror=1>')).toBe('&lt;img src=x onerror=1&gt;');
    } finally {
      vi.doUnmock('dompurify');
      vi.resetModules();
    }
  });
});

describe('escaping helpers', () => {
  it('escapes text', () => {
    expect(escapeHtml('<a> & b')).toBe('&lt;a&gt; &amp; b');
  });

  it('escapes both kinds of quote for attributes', () => {
    expect(escapeAttribute(`"'<>&`)).toBe('&quot;&#39;&lt;&gt;&amp;');
  });

  it('accepts values that are not strings', () => {
    expect(escapeAttribute(undefined)).toBe('undefined');
    expect(escapeHtml(12)).toBe('12');
  });
});
