<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { EditorView, basicSetup } from 'codemirror';
  import { EditorState } from '@codemirror/state';
  import { javascript } from '@codemirror/lang-javascript';
  import { python } from '@codemirror/lang-python';
  import { markdown } from '@codemirror/lang-markdown';

  export let value: string = '';
  export let onChange: (value: string) => void = () => {};
  export let language: 'javascript' | 'python' | 'markdown' | 'plain' = 'plain';
  export let readonly: boolean = false;

  let editorContainer: HTMLDivElement;
  let editorView: EditorView | null = null;

  // Get language extension based on language prop
  function getLanguageExtension() {
    switch (language) {
      case 'javascript':
        return javascript();
      case 'python':
        return python();
      case 'markdown':
        return markdown();
      default:
        return [];
    }
  }

  onMount(() => {
    const extensions = [
      basicSetup,
      getLanguageExtension(),
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
</script>

<div bind:this={editorContainer} class="h-full w-full"></div>

<style>
  :global(.cm-editor) {
    height: 100%;
    background: transparent;
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

  :global(.cm-gutters) {
    background-color: transparent;
    border-right: 1px solid rgb(229 231 235);
  }

  :global(.dark .cm-gutters) {
    border-right-color: rgb(55 65 81);
  }

  :global(.cm-activeLineGutter) {
    background-color: rgb(243 244 246);
  }

  :global(.dark .cm-activeLineGutter) {
    background-color: rgb(31 41 55);
  }

  :global(.cm-activeLine) {
    background-color: rgb(249 250 251);
  }

  :global(.dark .cm-activeLine) {
    background-color: rgb(31 41 55);
  }

  :global(.cm-selectionBackground) {
    background-color: rgb(191 219 254) !important;
  }

  :global(.dark .cm-selectionBackground) {
    background-color: rgb(30 64 175) !important;
  }

  :global(.cm-cursor) {
    border-left-color: rgb(37 99 235);
  }

  :global(.dark .cm-cursor) {
    border-left-color: rgb(96 165 250);
  }
</style>
