<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { EditorView, minimalSetup } from 'codemirror';
  import { EditorState, Compartment } from '@codemirror/state';
  import { javascript } from '@codemirror/lang-javascript';
  import { python } from '@codemirror/lang-python';
  import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
  import { json } from '@codemirror/lang-json';
  import { html } from '@codemirror/lang-html';
  import { css } from '@codemirror/lang-css';
  import { sql } from '@codemirror/lang-sql';
  import { StreamLanguage, LanguageDescription } from '@codemirror/language';
  import { perl } from '@codemirror/legacy-modes/mode/perl';
  import { shell } from '@codemirror/legacy-modes/mode/shell';
  import { oneDark } from '@codemirror/theme-one-dark';
  import { lineNumbers } from '@codemirror/view';
  import { highlightActiveLine, highlightSpecialChars } from '@codemirror/view';
  import { history, historyKeymap } from '@codemirror/commands';
  import { searchKeymap, highlightSelectionMatches } from '@codemirror/search';
  import { autocompletion, completionKeymap } from '@codemirror/autocomplete';
  import { bracketMatching } from '@codemirror/language';
  import { defaultKeymap, indentWithTab } from '@codemirror/commands';
  import { keymap } from '@codemirror/view';

  export let value: string = '';
  export let onChange: (value: string) => void = () => {};
  export let language: 'plain' | 'javascript' | 'python' | 'markdown' | 'json' | 'html' | 'css' | 'sql' | 'bash' | 'perl' = 'plain';
  export let readonly: boolean = false;
  export let wordWrap: boolean = true;
  export let isDark: boolean = false;

  // Export focus method for parent components to call
  export function focus() {
    if (editorView) {
      editorView.focus();
    }
  }

  let editorContainer: HTMLDivElement;
  let editorWrapper: HTMLDivElement;
  let editorView: EditorView | null = null;
  let languageCompartment = new Compartment();
  let wrapCompartment = new Compartment();
  let themeCompartment = new Compartment();
  let measuredWidth: number = 0;

  // Get language extension based on language prop
  function getLanguageExtension() {
    switch (language) {
      case 'javascript':
        return javascript();
      case 'python':
        return python();
      case 'markdown':
        // Configure markdown with syntax highlighting for code blocks
        return markdown({
          base: markdownLanguage,
          codeLanguages: [
            LanguageDescription.of({
              name: 'javascript',
              alias: ['js', 'jsx', 'ts', 'typescript'],
              load: async () => javascript()
            }),
            LanguageDescription.of({
              name: 'python',
              alias: ['py'],
              load: async () => python()
            }),
            LanguageDescription.of({
              name: 'json',
              load: async () => json()
            }),
            LanguageDescription.of({
              name: 'html',
              load: async () => html()
            }),
            LanguageDescription.of({
              name: 'css',
              load: async () => css()
            }),
            LanguageDescription.of({
              name: 'sql',
              load: async () => sql()
            }),
            LanguageDescription.of({
              name: 'bash',
              alias: ['sh', 'shell'],
              load: async () => StreamLanguage.define(shell)
            }),
            LanguageDescription.of({
              name: 'perl',
              load: async () => StreamLanguage.define(perl)
            }),
          ]
        });
      case 'json':
        return json();
      case 'html':
        return html();
      case 'css':
        return css();
      case 'sql':
        return sql();
      case 'bash':
        return StreamLanguage.define(shell);
      case 'perl':
        return StreamLanguage.define(perl);
      default:
        return [];
    }
  }

  onMount(() => {
    // Measure the available width
    measuredWidth = editorWrapper.clientWidth;

    const extensions = [
      // Custom setup without drawSelection - use native browser selection instead
      lineNumbers(),
      highlightActiveLine(),
      highlightSpecialChars(),
      history(),
      bracketMatching(),
      autocompletion(),
      highlightSelectionMatches(),
      keymap.of([
        ...defaultKeymap,
        ...historyKeymap,
        ...searchKeymap,
        ...completionKeymap,
        indentWithTab
      ]),
      // Our custom extensions
      languageCompartment.of(getLanguageExtension()),
      wrapCompartment.of(wordWrap ? EditorView.lineWrapping : []),
      themeCompartment.of(isDark ? oneDark : []),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          const newValue = update.state.doc.toString();
          if (newValue !== value) {
            onChange(newValue);
          }
        }
      }),
      EditorView.editable.of(!readonly),
    ];

    const startState = EditorState.create({
      doc: value,
      extensions,
    });

    editorView = new EditorView({
      state: startState,
      parent: editorContainer,
    });

    // Update width on window resize
    const handleResize = () => {
      measuredWidth = editorWrapper.clientWidth;
    };
    window.addEventListener('resize', handleResize);

    return () => {
      window.removeEventListener('resize', handleResize);
    };
  });

  onDestroy(() => {
    if (editorView) {
      editorView.destroy();
      editorView = null;
    }
  });

  // Update editor when value prop changes externally
  $: if (editorView && value !== editorView.state.doc.toString()) {
    editorView.dispatch({
      changes: {
        from: 0,
        to: editorView.state.doc.length,
        insert: value,
      },
    });
  }

  // Update language when language prop changes
  $: if (editorView && language) {
    editorView.dispatch({
      effects: languageCompartment.reconfigure(getLanguageExtension()),
    });
  }

  // Update word wrap when wordWrap prop changes
  $: if (editorView && wordWrap !== undefined) {
    editorView.dispatch({
      effects: wrapCompartment.reconfigure(wordWrap ? EditorView.lineWrapping : []),
    });
  }

  // Update theme when isDark prop changes
  $: if (editorView && isDark !== undefined) {
    editorView.dispatch({
      effects: themeCompartment.reconfigure(isDark ? oneDark : []),
    });
  }
</script>

<div bind:this={editorWrapper} class="h-full w-full overflow-hidden">
  <div
    bind:this={editorContainer}
    class="h-full"
    style="width: {measuredWidth}px; max-width: {measuredWidth}px; overflow-x: auto; overflow-y: hidden;"
  ></div>
</div>

<style>
  :global(.cm-editor) {
    height: 100%;
  }

  :global(.cm-scroller) {
    overflow-y: auto;
    overflow-x: hidden;
    font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
    font-size: 14px;
    line-height: 1.6;
    /* Mobile-friendly scrolling */
    -webkit-overflow-scrolling: touch;
    overscroll-behavior: contain;
  }

  :global(.cm-content) {
    padding: 0.5rem; /* Reduced padding for mobile */
    min-height: 100%;
  }

  /* Larger padding on tablet and desktop */
  @media (min-width: 768px) {
    :global(.cm-content) {
      padding: 1rem;
    }
  }

  /* Mobile-friendly scrollbars */
  @media (max-width: 767px) {
    :global(.cm-scroller::-webkit-scrollbar) {
      width: 8px;
      height: 8px;
    }

    :global(.cm-scroller::-webkit-scrollbar-track) {
      background: transparent;
    }

    :global(.cm-scroller::-webkit-scrollbar-thumb) {
      background-color: rgba(0, 0, 0, 0.2);
      border-radius: 4px;
    }

    :global(.dark .cm-scroller::-webkit-scrollbar-thumb) {
      background-color: rgba(255, 255, 255, 0.2);
    }

    /* Hide scrollbar when not scrolling on mobile */
    :global(.cm-scroller::-webkit-scrollbar-thumb) {
      opacity: 0;
      transition: opacity 0.3s;
    }

    :global(.cm-scroller:hover::-webkit-scrollbar-thumb) {
      opacity: 1;
    }
  }

  /* Light mode styles (when oneDark is not active) */
  :global(.cm-editor:not(.cm-theme-dark)) {
    background: white;
  }

  :global(.dark .cm-editor:not(.cm-theme-dark)) {
    background: rgb(17 24 39); /* gray-900 */
  }

  :global(.cm-editor:not(.cm-theme-dark) .cm-gutters) {
    background-color: rgb(249 250 251); /* gray-50 */
    border-right: 1px solid rgb(229 231 235); /* gray-200 */
    color: rgb(107 114 128); /* gray-500 */
  }

  :global(.dark .cm-editor:not(.cm-theme-dark) .cm-gutters) {
    background-color: rgb(17 24 39); /* gray-900 */
    border-right-color: rgb(55 65 81); /* gray-700 */
    color: rgb(156 163 175); /* gray-400 */
  }

  :global(.cm-editor:not(.cm-theme-dark) .cm-activeLineGutter) {
    background-color: rgb(243 244 246); /* gray-100 */
  }

  :global(.dark .cm-editor:not(.cm-theme-dark) .cm-activeLineGutter) {
    background-color: rgb(31 41 55); /* gray-800 */
  }

  :global(.cm-editor:not(.cm-theme-dark) .cm-activeLine) {
    background-color: rgb(249 250 251); /* gray-50 */
  }

  :global(.dark .cm-editor:not(.cm-theme-dark) .cm-activeLine) {
    background-color: rgb(31 41 55); /* gray-800 */
  }

  :global(.dark .cm-editor:not(.cm-theme-dark) .cm-content) {
    color: rgb(243 244 246); /* gray-100 */
  }

  /* Use native browser selection - much more reliable */
  :global(.cm-content ::selection) {
    background-color: #b3d7ff;
  }

  :global(.dark .cm-content ::selection) {
    background-color: #264f78;
  }

  :global(.cm-content ::-moz-selection) {
    background-color: #b3d7ff;
  }

  :global(.dark .cm-content ::-moz-selection) {
    background-color: #264f78;
  }
</style>
