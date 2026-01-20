<script lang="ts">
  import { locale, _ } from 'svelte-i18n';
  import { marked } from 'marked';
  import { toast } from '../utils/toast.svelte';
  import { getHljsInstance } from '../utils/syntaxHighlighter';
  import { loadDocumentation, defaultDocumentation } from '../docs';
  import { modal, createBackdropHandler } from '../actions';

  const hljs = getHljsInstance();

  export let show = false;
  export let onClose: () => void = () => {};

  $: backdropHandler = createBackdropHandler(onClose);

  // Documentation content loaded based on locale
  let documentationMarkdown = defaultDocumentation;
  let currentLocale = '';

  // Load documentation when locale changes or modal opens
  $: if (show && $locale && $locale !== currentLocale) {
    currentLocale = $locale;
    loadDocumentation($locale).then(doc => {
      documentationMarkdown = doc;
    });
  }

  // Configure marked for rendering
  function renderMarkdown(text: string): string {
    try {
      marked.setOptions({
        breaks: true,
        gfm: true,
      });

      const renderer = new marked.Renderer();

      // Add syntax highlighting to code blocks
      renderer.code = function(token) {
        const code = token.text;
        const language = token.lang;
        if (language && hljs.getLanguage(language)) {
          const highlighted = hljs.highlight(code, { language }).value;
          return `<pre class="hljs rounded-lg p-4 overflow-x-auto"><code class="language-${language}">${highlighted}</code></pre>`;
        }
        return `<pre class="bg-gray-100 dark:bg-gray-900 rounded-lg p-4 overflow-x-auto"><code>${code}</code></pre>`;
      };

      // Add IDs to headings for TOC links
      renderer.heading = function(token) {
        const text = token.text;
        const depth = token.depth;
        const id = text.toLowerCase()
          .replace(/[^\w\s-]/g, '')  // Remove special characters
          .replace(/\s+/g, '-')       // Replace spaces with hyphens
          .replace(/-+/g, '-')        // Collapse multiple hyphens
          .replace(/^-|-$/g, '');     // Trim leading/trailing hyphens
        return `<h${depth} id="${id}" class="scroll-mt-4">${text}</h${depth}>`;
      };

      // Style links
      renderer.link = function(token) {
        const href = token.href;
        const text = token.text;
        if (href.startsWith('#')) {
          // Internal anchor link
          return `<a href="${href}" class="text-blue-600 dark:text-blue-400 hover:underline">${text}</a>`;
        }
        return `<a href="${href}" target="_blank" rel="noopener noreferrer" class="text-blue-600 dark:text-blue-400 hover:underline">${text}</a>`;
      };

      return marked.parse(text, { renderer }) as string;
    } catch (error) {
      console.error('Markdown parsing error:', error);
      return `<p>Error rendering documentation</p>`;
    }
  }

  // Copy documentation as markdown
  async function copyMarkdown() {
    try {
      await navigator.clipboard.writeText(documentationMarkdown);
      toast.success($_('docs.copied'));
    } catch (error) {
      console.error('Failed to copy:', error);
      toast.error($_('docs.copyError'));
    }
  }

  $: renderedHtml = renderMarkdown(documentationMarkdown);

  // Reference to the scrollable content container
  let contentContainer: HTMLDivElement;

  // Normalize an anchor ID (same logic as heading ID generation)
  function normalizeAnchorId(id: string): string {
    return id.toLowerCase()
      .replace(/[^\w\s-]/g, '')   // Remove special characters
      .replace(/\s+/g, '-')        // Replace spaces with hyphens
      .replace(/-+/g, '-')         // Collapse multiple hyphens
      .replace(/^-|-$/g, '');      // Trim leading/trailing hyphens
  }

  // Handle anchor link clicks for internal navigation
  function handleContentClick(e: MouseEvent) {
    const target = e.target as HTMLElement;
    const link = target.closest('a');
    if (link) {
      const href = link.getAttribute('href');
      if (href?.startsWith('#')) {
        e.preventDefault();
        // Normalize the ID to match our heading ID generation
        const id = normalizeAnchorId(href.slice(1));
        // Search within this container only (not the whole document)
        const element = contentContainer?.querySelector(`#${id}`) as HTMLElement;
        if (element && contentContainer) {
          // Calculate position relative to the scroll container
          const containerRect = contentContainer.getBoundingClientRect();
          const elementRect = element.getBoundingClientRect();
          const scrollTop = contentContainer.scrollTop + (elementRect.top - containerRect.top) - 20;
          contentContainer.scrollTo({ top: scrollTop, behavior: 'smooth' });
        }
      }
    }
  }
</script>

{#if show}
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-0 tablet:p-4"
    on:click={backdropHandler}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    use:modal={{ onEscape: onClose }}
  >
    <div class="bg-white dark:bg-gray-800 w-full h-full tablet:h-auto tablet:max-w-4xl tablet:rounded-lg shadow-xl tablet:max-h-[90vh] flex flex-col">
      <!-- Header -->
      <div class="border-b border-gray-200 dark:border-gray-700 p-4 flex items-center justify-between flex-shrink-0">
        <h2 class="text-xl font-bold text-gray-900 dark:text-white">{$_('docs.title')}</h2>
        <div class="flex items-center gap-2">
          <button
            on:click={copyMarkdown}
            class="min-h-11 px-3 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600 rounded-md transition-colors flex items-center gap-2"
            title={$_('docs.copyMarkdown')}
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
            </svg>
            {$_('docs.copy')}
          </button>
          <button
            on:click={onClose}
            class="min-h-11 min-w-11 p-3 -m-2 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors text-gray-500 dark:text-gray-400"
            aria-label="Close documentation"
          >
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
      </div>

      <!-- Content (scrollable) -->
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
      <div bind:this={contentContainer} class="flex-1 overflow-y-auto p-6" on:click={handleContentClick}>
        <article class="prose prose-sm dark:prose-invert max-w-none
          prose-headings:font-semibold
          prose-h1:text-2xl prose-h1:border-b prose-h1:border-gray-200 prose-h1:dark:border-gray-700 prose-h1:pb-2 prose-h1:mb-4
          prose-h2:text-xl prose-h2:mt-8 prose-h2:mb-4
          prose-h3:text-lg prose-h3:mt-6 prose-h3:mb-3
          prose-p:text-gray-700 prose-p:dark:text-gray-300
          prose-a:text-blue-600 prose-a:dark:text-blue-400
          prose-code:bg-gray-100 prose-code:dark:bg-gray-900 prose-code:px-1.5 prose-code:py-0.5 prose-code:rounded prose-code:text-sm prose-code:before:content-none prose-code:after:content-none
          prose-pre:bg-gray-100 prose-pre:dark:bg-gray-900 prose-pre:p-0
          prose-table:text-sm
          prose-th:bg-gray-100 prose-th:dark:bg-gray-700 prose-th:px-3 prose-th:py-2 prose-th:text-left prose-th:font-medium
          prose-td:px-3 prose-td:py-2 prose-td:border-t prose-td:border-gray-200 prose-td:dark:border-gray-700
          prose-blockquote:border-l-4 prose-blockquote:border-blue-500 prose-blockquote:bg-blue-50 prose-blockquote:dark:bg-blue-900/20 prose-blockquote:py-2 prose-blockquote:px-4 prose-blockquote:not-italic
          prose-hr:border-gray-200 prose-hr:dark:border-gray-700
          prose-ul:list-disc prose-ol:list-decimal
          prose-li:text-gray-700 prose-li:dark:text-gray-300
          prose-strong:text-gray-900 prose-strong:dark:text-white
        ">
          {@html renderedHtml}
        </article>
      </div>

      <!-- Footer -->
      <div class="border-t border-gray-200 dark:border-gray-700 p-4 flex justify-end flex-shrink-0">
        <button
          on:click={onClose}
          class="px-4 py-2.5 min-h-11 bg-blue-600 active:bg-blue-700 text-white font-medium rounded-md transition-colors"
        >
          {$_('common.close')}
        </button>
      </div>
    </div>
  </div>
{/if}
