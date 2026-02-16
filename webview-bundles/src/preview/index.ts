/**
 * Rich markdown preview bundle for iOS WKWebView.
 *
 * Uses marked.js for markdown parsing and highlight.js for syntax highlighting
 * in fenced code blocks. Resolves attachment:uuid image references via the
 * Swift bridge, and intercepts link clicks to delegate to Swift.
 */

import { marked } from 'marked';
import hljs from 'highlight.js/lib/core';
import { postToSwift } from '../bridge';

// Import only the languages used most commonly in notes
import javascript from 'highlight.js/lib/languages/javascript';
import typescript from 'highlight.js/lib/languages/typescript';
import python from 'highlight.js/lib/languages/python';
import bash from 'highlight.js/lib/languages/bash';
import json from 'highlight.js/lib/languages/json';
import xml from 'highlight.js/lib/languages/xml';
import css from 'highlight.js/lib/languages/css';
import sql from 'highlight.js/lib/languages/sql';
import markdown from 'highlight.js/lib/languages/markdown';
import yaml from 'highlight.js/lib/languages/yaml';
import rust from 'highlight.js/lib/languages/rust';
import go from 'highlight.js/lib/languages/go';
import java from 'highlight.js/lib/languages/java';
import swift from 'highlight.js/lib/languages/swift';
import perl from 'highlight.js/lib/languages/perl';
import ruby from 'highlight.js/lib/languages/ruby';
import diff from 'highlight.js/lib/languages/diff';
import dockerfile from 'highlight.js/lib/languages/dockerfile';
import plaintext from 'highlight.js/lib/languages/plaintext';

hljs.registerLanguage('javascript', javascript);
hljs.registerLanguage('js', javascript);
hljs.registerLanguage('typescript', typescript);
hljs.registerLanguage('ts', typescript);
hljs.registerLanguage('python', python);
hljs.registerLanguage('bash', bash);
hljs.registerLanguage('sh', bash);
hljs.registerLanguage('shell', bash);
hljs.registerLanguage('json', json);
hljs.registerLanguage('xml', xml);
hljs.registerLanguage('html', xml);
hljs.registerLanguage('css', css);
hljs.registerLanguage('sql', sql);
hljs.registerLanguage('markdown', markdown);
hljs.registerLanguage('md', markdown);
hljs.registerLanguage('yaml', yaml);
hljs.registerLanguage('yml', yaml);
hljs.registerLanguage('rust', rust);
hljs.registerLanguage('go', go);
hljs.registerLanguage('java', java);
hljs.registerLanguage('swift', swift);
hljs.registerLanguage('perl', perl);
hljs.registerLanguage('ruby', ruby);
hljs.registerLanguage('rb', ruby);
hljs.registerLanguage('diff', diff);
hljs.registerLanguage('dockerfile', dockerfile);
hljs.registerLanguage('plaintext', plaintext);

// Import highlight.js styles — light by default, dark overridden in our CSS
import 'highlight.js/styles/github.css';

// ---- State ----

let isDark = false;
const pendingAttachments = new Map<string, HTMLElement[]>();

// ---- Theme ----

function applyTheme(dark: boolean): void {
  isDark = dark;
  document.documentElement.classList.toggle('dark', dark);
}

// ---- Attachment resolution ----

function onRequestAttachment(id: string, element: HTMLElement): void {
  const list = pendingAttachments.get(id) ?? [];
  list.push(element);
  pendingAttachments.set(id, list);
  postToSwift({ type: 'requestAttachment', id });
}

// ---- Markdown rendering ----

function renderMarkdown(content: string): string {
  const renderer = new marked.Renderer();

  // Custom heading renderer with anchor IDs
  renderer.heading = function (token) {
    const text = token.text;
    const depth = token.depth;
    const id = text
      .toLowerCase()
      .replace(/[^\w\s-]/g, '')
      .replace(/\s+/g, '-')
      .replace(/-+/g, '-')
      .replace(/^-|-$/g, '');
    return `<h${depth} id="${id}">${text}</h${depth}>`;
  };

  // Custom code block renderer with syntax highlighting
  renderer.code = function (token) {
    const code = token.text;
    const language = token.lang;

    if (language && hljs.getLanguage(language)) {
      try {
        const highlighted = hljs.highlight(code, { language }).value;
        return `<pre><code class="hljs language-${language}">${highlighted}</code></pre>`;
      } catch {
        // Fall through to auto-detect
      }
    }

    if (code) {
      try {
        const highlighted = hljs.highlightAuto(code).value;
        return `<pre><code class="hljs">${highlighted}</code></pre>`;
      } catch {
        // Fall through to plain
      }
    }

    const escaped = code.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    return `<pre><code>${escaped}</code></pre>`;
  };

  // Custom image renderer to handle attachment: URLs
  renderer.image = function (token) {
    const href = token.href;
    const alt = token.text || '';
    const title = token.title || '';

    if (href && href.startsWith('attachment:')) {
      const attachmentId = href.substring('attachment:'.length);
      const titleAttr = title ? ` title="${title}"` : '';
      // Render a placeholder that will be resolved by Swift
      return `<img src="" data-attachment-id="${attachmentId}" alt="${alt}"${titleAttr} class="attachment-image" style="max-width:100%;height:auto;display:none;" />`;
    }

    const titleAttr = title ? ` title="${title}"` : '';
    return `<img src="${href}" alt="${alt}"${titleAttr} style="max-width:100%;height:auto;" />`;
  };

  // Custom link renderer to handle attachment: and note: URLs
  renderer.link = function (token) {
    const href = token.href;
    const text = token.text || '';

    // Attachment download link
    if (href && href.startsWith('attachment:')) {
      const attachmentId = href.substring('attachment:'.length);
      return `<span class="attachment-link" data-attachment-id="${attachmentId}">${text || attachmentId}</span>`;
    }

    // Note link (from [[wiki-style]] conversion)
    if (href && (href.startsWith('note:') || href.startsWith('link:'))) {
      return `<span class="note-link">${text}</span>`;
    }

    // External link - delegate to Swift on click
    const titleAttr = token.title ? ` title="${token.title}"` : '';
    return `<a href="${href}"${titleAttr} class="external-link">${text}</a>`;
  };

  marked.setOptions({ breaks: true, gfm: true });
  return marked.parse(content, { renderer }) as string;
}

// ---- Content rendering ----

function setContent(content: string, dark: boolean): void {
  applyTheme(dark);
  pendingAttachments.clear();

  const container = document.getElementById('content')!;
  container.innerHTML = renderMarkdown(content);

  // Request resolution for all attachment images
  container.querySelectorAll<HTMLImageElement>('img[data-attachment-id]').forEach((img) => {
    const id = img.dataset.attachmentId!;
    onRequestAttachment(id, img);
  });

  // Request resolution for attachment links
  container.querySelectorAll<HTMLElement>('.attachment-link[data-attachment-id]').forEach((el) => {
    const id = el.dataset.attachmentId!;
    onRequestAttachment(id, el);
  });

  // Set up click handlers for external links
  container.querySelectorAll<HTMLAnchorElement>('a.external-link').forEach((a) => {
    a.addEventListener('click', (e) => {
      e.preventDefault();
      const url = a.getAttribute('href');
      if (url) {
        postToSwift({ type: 'openLink', url });
      }
    });
  });

  // Handle internal anchor links
  container.querySelectorAll<HTMLAnchorElement>('a[href^="#"]').forEach((a) => {
    a.addEventListener('click', (e) => {
      e.preventDefault();
      const targetId = a.getAttribute('href')?.substring(1);
      if (targetId) {
        const target = document.getElementById(targetId);
        target?.scrollIntoView({ behavior: 'smooth' });
      }
    });
  });
}

// ---- Bridge API ----

window.bridge = {
  setContent,
  setTheme: applyTheme,
  setFontSize(px: number): void {
    document.body.style.fontSize = `${px}px`;
  },
  resolveAttachment(id: string, dataUrl: string): void {
    const elements = pendingAttachments.get(id);
    if (!elements) return;

    for (const el of elements) {
      if (el instanceof HTMLImageElement) {
        el.src = dataUrl;
        el.style.display = '';
      }
    }
    pendingAttachments.delete(id);
  },
};

// ---- Styles ----

const style = document.createElement('style');
style.textContent = `
  * { box-sizing: border-box; }
  :root {
    --bg: #ffffff;
    --fg: #1c1c1e;
    --code-bg: #f2f2f7;
    --code-fg: #1c1c1e;
    --link: #007aff;
    --hr: #d1d1d6;
    --bq-border: #c7c7cc;
    --bq-bg: #f9f9f9;
    --table-border: #d1d1d6;
    --table-stripe: #f8f8f8;
  }
  .dark {
    --bg: #1c1c1e;
    --fg: #e5e5e7;
    --code-bg: #2c2c2e;
    --code-fg: #e5e5e7;
    --link: #64d2ff;
    --hr: #48484a;
    --bq-border: #636366;
    --bq-bg: #2c2c2e;
    --table-border: #48484a;
    --table-stripe: #2c2c2e;
  }
  body {
    font-family: -apple-system, BlinkMacSystemFont, sans-serif;
    font-size: 16px;
    line-height: 1.6;
    color: var(--fg);
    background-color: var(--bg);
    padding: 16px;
    margin: 0;
    word-wrap: break-word;
    overflow-wrap: break-word;
    -webkit-text-size-adjust: 100%;
  }
  h1, h2, h3, h4, h5, h6 {
    margin-top: 1.2em;
    margin-bottom: 0.4em;
    font-weight: 600;
    line-height: 1.3;
  }
  h1 { font-size: 1.8em; border-bottom: 1px solid var(--hr); padding-bottom: 0.3em; }
  h2 { font-size: 1.5em; border-bottom: 1px solid var(--hr); padding-bottom: 0.3em; }
  h3 { font-size: 1.25em; }
  h4 { font-size: 1.1em; }
  h5 { font-size: 1em; }
  h6 { font-size: 0.9em; opacity: 0.8; }
  p { margin: 0.6em 0; }
  a { color: var(--link); text-decoration: none; }
  a:hover { text-decoration: underline; }
  code {
    font-family: 'SF Mono', Menlo, monospace;
    font-size: 0.88em;
    background: var(--code-bg);
    color: var(--code-fg);
    padding: 0.15em 0.35em;
    border-radius: 4px;
  }
  pre {
    background: var(--code-bg);
    color: var(--code-fg);
    padding: 12px 16px;
    border-radius: 8px;
    overflow-x: auto;
    -webkit-overflow-scrolling: touch;
    margin: 0.8em 0;
    line-height: 1.45;
  }
  pre code {
    background: none;
    padding: 0;
    border-radius: 0;
    font-size: 0.85em;
  }
  blockquote {
    margin: 0.8em 0;
    padding: 0.4em 1em;
    border-left: 4px solid var(--bq-border);
    background: var(--bq-bg);
    border-radius: 0 4px 4px 0;
  }
  blockquote p { margin: 0.3em 0; }
  ul, ol { padding-left: 1.5em; margin: 0.6em 0; }
  li { margin: 0.2em 0; }
  li > ul, li > ol { margin: 0.1em 0; }
  hr {
    border: none;
    border-top: 1px solid var(--hr);
    margin: 1.5em 0;
  }
  table {
    border-collapse: collapse;
    width: 100%;
    margin: 0.8em 0;
    font-size: 0.95em;
  }
  th, td {
    border: 1px solid var(--table-border);
    padding: 8px 12px;
    text-align: left;
  }
  th {
    font-weight: 600;
    background: var(--code-bg);
  }
  tr:nth-child(even) { background: var(--table-stripe); }
  img { max-width: 100%; height: auto; border-radius: 4px; }
  del { opacity: 0.6; }
  input[type="checkbox"] { margin-right: 0.4em; }
  .attachment-link {
    display: inline-flex;
    align-items: center;
    padding: 2px 8px;
    background: var(--code-bg);
    border-radius: 4px;
    font-size: 0.9em;
    color: var(--link);
    cursor: pointer;
  }
  .attachment-link::before { content: '📎 '; }
  .note-link {
    display: inline-flex;
    align-items: center;
    padding: 2px 8px;
    background: var(--code-bg);
    border-radius: 4px;
    font-size: 0.9em;
    color: var(--link);
  }
  .note-link::before { content: '📄 '; }

  /* Override highlight.js background to match our theme */
  .hljs { background: transparent !important; }

  /* Dark mode highlight.js overrides (github-dark palette) */
  .dark .hljs { color: #e6edf3; }
  .dark .hljs-keyword,
  .dark .hljs-selector-tag { color: #ff7b72; }
  .dark .hljs-string,
  .dark .hljs-addition { color: #a5d6ff; }
  .dark .hljs-comment,
  .dark .hljs-quote { color: #8b949e; }
  .dark .hljs-number,
  .dark .hljs-literal { color: #79c0ff; }
  .dark .hljs-title,
  .dark .hljs-section,
  .dark .hljs-title.function_ { color: #d2a8ff; }
  .dark .hljs-type,
  .dark .hljs-built_in { color: #ffa657; }
  .dark .hljs-variable,
  .dark .hljs-template-variable { color: #ffa657; }
  .dark .hljs-attr,
  .dark .hljs-attribute { color: #79c0ff; }
  .dark .hljs-symbol,
  .dark .hljs-bullet { color: #ffa657; }
  .dark .hljs-deletion { color: #ffa198; background: #490202; }
  .dark .hljs-addition { background: #04260f; }
  .dark .hljs-emphasis { font-style: italic; }
  .dark .hljs-strong { font-weight: bold; }
`;
document.head.appendChild(style);

// ---- Init ----

postToSwift({ type: 'ready' });
