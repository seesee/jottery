<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { EditorView } from 'codemirror';
  import { EditorState, Compartment } from '@codemirror/state';
  import { javascript } from '@codemirror/lang-javascript';
  import { python } from '@codemirror/lang-python';
  import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
  import { json } from '@codemirror/lang-json';
  import { html } from '@codemirror/lang-html';
  import { css } from '@codemirror/lang-css';
  import { sql } from '@codemirror/lang-sql';
  import { StreamLanguage, LanguageDescription, LanguageSupport } from '@codemirror/language';
  import { perl } from '@codemirror/legacy-modes/mode/perl';
  import { shell } from '@codemirror/legacy-modes/mode/shell';
  import { calcExtension } from '../extensions/calcExtension';
  import { oneDark } from '@codemirror/theme-one-dark';
  import { lineNumbers } from '@codemirror/view';
  import { highlightActiveLine, highlightSpecialChars } from '@codemirror/view';
  import { history, historyKeymap, undo as undoCommand, redo as redoCommand } from '@codemirror/commands';
  import { searchKeymap, highlightSelectionMatches } from '@codemirror/search';
  import { autocompletion, completionKeymap } from '@codemirror/autocomplete';
  import { bracketMatching } from '@codemirror/language';
  import { defaultKeymap, indentWithTab } from '@codemirror/commands';
  import { keymap } from '@codemirror/view';
  import { settings } from '../stores/appStore';
  import { getFontSize } from '../utils/fontSize';

  export let value: string = '';
  export let onChange: (value: string) => void = () => {};
  export let language: string | undefined = 'plain';
  export let readonly: boolean = false;
  export let wordWrap: boolean = true;
  export let isDark: boolean = false;
  export let onImagePaste: ((file: File) => Promise<string | null>) | undefined = undefined;

  // Export focus method for parent components to call
  export function focus() {
    if (editorView) {
      editorView.focus();
    }
  }

  // Export undo method
  export function undo() {
    if (editorView) {
      undoCommand(editorView);
    }
  }

  // Export redo method
  export function redo() {
    if (editorView) {
      redoCommand(editorView);
    }
  }

  let editorContainer: HTMLDivElement;
  let editorWrapper: HTMLDivElement;
  let editorView: EditorView | null = null;
  let languageCompartment = new Compartment();
  let wrapCompartment = new Compartment();
  let themeCompartment = new Compartment();
  let mobileAttributesCompartment = new Compartment();
  let measuredWidth: number = 0;

  // Compute font size from settings
  $: fontSize = getFontSize($settings.fontSize);

  // Get mobile keyboard attributes based on language (only for prose, not code)
  function getMobileAttributes() {
    // Only enable autocorrect/autocapitalize for prose languages (plain text and markdown)
    const isProse = language === 'plain' || language === 'markdown';

    return EditorView.contentAttributes.of({
      autocorrect: isProse ? 'on' : 'off',
      autocapitalize: isProse ? 'on' : 'off',
      spellcheck: isProse ? 'true' : 'false',
    });
  }

  // Get language extension based on language prop
  function getLanguageExtension() {
    switch (language) {
      case 'calc':
        return calcExtension(isDark);
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
              load: async () => new LanguageSupport(StreamLanguage.define(shell))
            }),
            LanguageDescription.of({
              name: 'perl',
              load: async () => new LanguageSupport(StreamLanguage.define(perl))
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
        return new LanguageSupport(StreamLanguage.define(shell));
      case 'perl':
        return new LanguageSupport(StreamLanguage.define(perl));
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
      mobileAttributesCompartment.of(getMobileAttributes()),
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
      // Handle paste events for images
      EditorView.domEventHandlers({
        paste: (event, view) => {
          if (!onImagePaste) return false;

          const clipboardData = event.clipboardData;
          if (!clipboardData) return false;

          // Check for image files in clipboard
          const items = Array.from(clipboardData.items);
          const imageItem = items.find(item => item.type.startsWith('image/'));

          if (!imageItem) return false;

          const file = imageItem.getAsFile();
          if (!file) return false;

          // Prevent default paste behaviour
          event.preventDefault();

          // Handle the image paste asynchronously
          (async () => {
            try {
              const markdownRef = await onImagePaste(file);
              if (markdownRef) {
                // Insert the markdown reference at cursor position
                const pos = view.state.selection.main.head;
                view.dispatch({
                  changes: { from: pos, insert: markdownRef },
                  selection: { anchor: pos + markdownRef.length },
                });
              }
            } catch (error) {
              console.error('Failed to handle image paste:', error);
            }
          })();

          return true; // Event handled
        },
      }),
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
      effects: [
        languageCompartment.reconfigure(getLanguageExtension()),
        mobileAttributesCompartment.reconfigure(getMobileAttributes()),
      ],
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
    const effects = [themeCompartment.reconfigure(isDark ? oneDark : [])];

    // Also reconfigure calc extension for theme-aware syntax highlighting
    if (language === 'calc') {
      effects.push(languageCompartment.reconfigure(getLanguageExtension()));
    }

    editorView.dispatch({ effects });
  }
</script>

<div bind:this={editorWrapper} class="h-full w-full overflow-hidden" style="font-size: {fontSize}px;">
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
    /* font-size inherited from parent wrapper div */
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
