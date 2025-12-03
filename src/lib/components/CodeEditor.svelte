<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { EditorView, basicSetup } from 'codemirror';
  import { EditorState, Compartment } from '@codemirror/state';
  import { javascript } from '@codemirror/lang-javascript';
  import { python } from '@codemirror/lang-python';
  import { markdown } from '@codemirror/lang-markdown';
  import { json } from '@codemirror/lang-json';
  import { html } from '@codemirror/lang-html';
  import { css } from '@codemirror/lang-css';
  import { sql } from '@codemirror/lang-sql';
  import { oneDark } from '@codemirror/theme-one-dark';

  export let value: string = '';
  export let onChange: (value: string) => void = () => {};
  export let language: 'plain' | 'javascript' | 'python' | 'markdown' | 'json' | 'html' | 'css' | 'sql' | 'bash' = 'plain';
  export let readonly: boolean = false;
  export let wordWrap: boolean = true;
  export let isDark: boolean = false;

  let editorContainer: HTMLDivElement;
  let editorView: EditorView | null = null;
  let languageCompartment = new Compartment();
  let wrapCompartment = new Compartment();
  let themeCompartment = new Compartment();

  // Get language extension based on language prop
  function getLanguageExtension() {
    switch (language) {
      case 'javascript':
        return javascript();
      case 'python':
        return python();
      case 'markdown':
        return markdown();
      case 'json':
        return json();
      case 'html':
        return html();
      case 'css':
        return css();
      case 'sql':
        return sql();
      case 'bash':
        // Use javascript for bash as @codemirror/lang-bash doesn't exist
        return javascript();
      default:
        return [];
    }
  }

  onMount(() => {
    const extensions = [
      basicSetup,
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
  $: if (editorView) {
    editorView.dispatch({
      effects: languageCompartment.reconfigure(getLanguageExtension()),
    });
  }

  // Update word wrap when wordWrap prop changes
  $: if (editorView) {
    editorView.dispatch({
      effects: wrapCompartment.reconfigure(wordWrap ? EditorView.lineWrapping : []),
    });
  }

  // Update theme when isDark prop changes
  $: if (editorView) {
    editorView.dispatch({
      effects: themeCompartment.reconfigure(isDark ? oneDark : []),
    });
  }
</script>

<div bind:this={editorContainer} class="h-full w-full"></div>

<style>
  :global(.cm-editor) {
    height: 100%;
  }

  :global(.cm-scroller) {
    overflow: auto;
    font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
    font-size: 14px;
    line-height: 1.6;
  }

  :global(.cm-content) {
    padding: 1rem;
    min-height: 100%;
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
</style>
