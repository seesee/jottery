/**
 * Calculator editor bundle for iOS WKWebView.
 *
 * Uses CodeMirror 6 + math.js to provide a REPL-style calculator editor
 * with inline result display, variable persistence, and syntax highlighting.
 * Adapted from the web app's calcExtension.ts.
 */

import { EditorView, ViewPlugin, Decoration, WidgetType, keymap } from '@codemirror/view';
import type { ViewUpdate, DecorationSet } from '@codemirror/view';
import { EditorState, Text, Range } from '@codemirror/state';
import { HighlightStyle, syntaxHighlighting, LanguageSupport, StreamLanguage } from '@codemirror/language';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { tags as t } from '@lezer/highlight';
import * as math from 'mathjs';
import { postToSwift } from '../bridge';

// ---- Constants ----

const RESULT_PREFIX = '  ';
const CALC_MAX_LINES = 500;
const CALC_MAX_TOTAL_ERRORS = 50;
const MAX_CONSECUTIVE_ERRORS = 10;

// ---- Math.js built-in identifiers ----

const BUILTIN_FUNCTIONS = new Set([
  'sin', 'cos', 'tan', 'asin', 'acos', 'atan', 'atan2',
  'sinh', 'cosh', 'tanh', 'asinh', 'acosh', 'atanh',
  'sqrt', 'cbrt', 'abs', 'ceil', 'floor', 'round', 'sign',
  'exp', 'log', 'log10', 'log2', 'ln',
  'min', 'max', 'mean', 'median', 'sum', 'prod',
  'random', 'randomInt',
  'gcd', 'lcm', 'mod', 'xgcd',
  'factorial', 'gamma', 'combinations', 'permutations',
]);

const BUILTIN_CONSTANTS = new Set([
  'pi', 'e', 'tau', 'phi', 'i', 'true', 'false', 'null', 'Infinity', 'NaN',
]);

// ---- Calc Language (StreamLanguage tokenizer) ----

const calcLanguage = StreamLanguage.define({
  token(stream) {
    if (stream.match(/^#.*/)) return 'comment';
    if (stream.match(/^[0-9]+\.?[0-9]*([eE][+-]?[0-9]+)?/)) return 'number';
    if (stream.match(/^[+\-*/%^=()[\],]/)) return 'operator';

    const wordMatch = stream.match(/^[\p{L}_][\p{L}\p{N}_]*/u);
    if (wordMatch && typeof wordMatch !== 'boolean') {
      const word = wordMatch[0];
      if (BUILTIN_FUNCTIONS.has(word)) return 'keyword';
      if (BUILTIN_CONSTANTS.has(word)) return 'atom';
      return 'variableName';
    }

    if (stream.match(/^\s+/)) return null;
    stream.next();
    return null;
  },
});

// ---- Syntax highlighting themes ----

const calcHighlightStyleDark = HighlightStyle.define([
  { tag: t.comment, color: '#6a9955' },
  { tag: t.number, color: '#b5cea8' },
  { tag: t.keyword, color: '#4ec9b0' },
  { tag: t.atom, color: '#569cd6' },
  { tag: t.variableName, color: '#9cdcfe' },
  { tag: t.operator, color: '#d4d4d4' },
]);

const calcHighlightStyleLight = HighlightStyle.define([
  { tag: t.comment, color: '#008000' },
  { tag: t.number, color: '#098658' },
  { tag: t.keyword, color: '#0070c1' },
  { tag: t.atom, color: '#0000ff' },
  { tag: t.variableName, color: '#001080' },
  { tag: t.operator, color: '#000000' },
]);

// ---- Parser ----

interface ParsedLine {
  lineNumber: number;
  expression: string;
  isComment: boolean;
  isAssignment: boolean;
  variable?: string;
}

class CalcParser {
  parseDocument(doc: Text): ParsedLine[] {
    const lines: ParsedLine[] = [];
    for (let i = 1; i <= doc.lines; i++) {
      const text = doc.line(i).text;
      if (!text.trim()) continue;

      if (text.trimStart().startsWith('#')) {
        lines.push({ lineNumber: i, expression: text, isComment: true, isAssignment: false });
        continue;
      }

      const match = text.match(/^\s*([\p{L}_][\p{L}\p{N}_]*)\s*=\s*(.+)$/u);
      if (match) {
        lines.push({ lineNumber: i, expression: text, isComment: false, isAssignment: true, variable: match[1] });
        continue;
      }

      lines.push({ lineNumber: i, expression: text, isComment: false, isAssignment: false });
    }
    return lines;
  }
}

// ---- Evaluator ----

interface EvaluationResult {
  lineNumber: number;
  result: string | null;
  isError: boolean;
}

class CalcEvaluator {
  private scope: Record<string, unknown> = {};
  private unicodeToAscii = new Map<string, string>();
  private varCounter = 0;

  reset(): void {
    this.scope = {};
    this.unicodeToAscii.clear();
    this.varCounter = 0;
  }

  private isUnicodeVar(name: string): boolean {
    return /[^\x00-\x7F]/.test(name);
  }

  private getAsciiAlias(unicodeName: string): string {
    if (!this.unicodeToAscii.has(unicodeName)) {
      this.unicodeToAscii.set(unicodeName, `_uvar${this.varCounter++}`);
    }
    return this.unicodeToAscii.get(unicodeName)!;
  }

  private rewriteExpression(expr: string): string {
    return expr.replace(/[\p{L}_][\p{L}\p{N}_]*/gu, (match) => {
      if (this.isUnicodeVar(match)) return this.getAsciiAlias(match);
      return match;
    });
  }

  evaluateLine(parsedLine: ParsedLine): EvaluationResult {
    if (parsedLine.isComment) {
      return { lineNumber: parsedLine.lineNumber, result: null, isError: false };
    }

    try {
      const rewrittenExpr = this.rewriteExpression(parsedLine.expression);
      const result = math.evaluate(rewrittenExpr, this.scope);
      return { lineNumber: parsedLine.lineNumber, result: this.formatResult(result), isError: false };
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      const truncated = message.length > 50 ? message.substring(0, 50) + '...' : message;
      return { lineNumber: parsedLine.lineNumber, result: `Error: ${truncated}`, isError: true };
    }
  }

  private formatResult(value: math.MathType): string {
    if (typeof value === 'number') {
      return Number.isInteger(value)
        ? value.toString()
        : value.toFixed(6).replace(/\.?0+$/, '');
    }
    return math.format(value, { precision: 14 });
  }
}

// ---- Widgets ----

class ResultWidget extends WidgetType {
  constructor(private result: string, private isError: boolean) {
    super();
  }

  toDOM(): HTMLElement {
    const span = document.createElement('span');
    span.textContent = RESULT_PREFIX + this.result;
    span.className = this.isError ? 'cm-calc-error' : 'cm-calc-result';
    span.setAttribute('aria-label', `equals ${this.result}`);
    return span;
  }

  eq(other: ResultWidget): boolean {
    return other.result === this.result && other.isError === this.isError;
  }
}

class WarningWidget extends WidgetType {
  constructor(private message: string) {
    super();
  }

  toDOM(): HTMLElement {
    const span = document.createElement('span');
    span.className = 'cm-calc-warning';
    span.textContent = '\u26A0 ' + this.message + ' ';
    return span;
  }

  eq(other: WarningWidget): boolean {
    return other.message === this.message;
  }
}

// ---- Decoration Builder ----

class DecorationBuilder {
  buildDecorations(results: EvaluationResult[], doc: Text, warning?: string): DecorationSet {
    const decorations: Range<Decoration>[] = [];

    if (warning && doc.lines >= 1) {
      const warningWidget = Decoration.widget({ widget: new WarningWidget(warning), side: -1 });
      decorations.push(warningWidget.range(0));
    }

    for (const result of results) {
      if (result.result === null) continue;
      const line = doc.line(result.lineNumber);

      if (result.isError) {
        const lineDeco = Decoration.line({ class: 'cm-calc-error-line' });
        decorations.push(lineDeco.range(line.from));
      } else {
        const widget = Decoration.widget({
          widget: new ResultWidget(result.result, result.isError),
          side: 1,
        });
        decorations.push(widget.range(line.to));
      }
    }

    return Decoration.set(decorations, true);
  }
}

// ---- Calc ViewPlugin ----

const calcPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    private parser = new CalcParser();
    private evaluator = new CalcEvaluator();
    private builder = new DecorationBuilder();

    constructor(view: EditorView) {
      this.decorations = this.compute(view);
    }

    update(update: ViewUpdate): void {
      if (!update.docChanged) return;
      this.decorations = this.compute(update.view);
    }

    compute(view: EditorView): DecorationSet {
      this.evaluator.reset();
      const doc = view.state.doc;
      const totalLines = doc.lines;

      if (totalLines > CALC_MAX_LINES) {
        const warning = `Document too long for calc mode (${totalLines} lines, max ${CALC_MAX_LINES}).`;
        return this.builder.buildDecorations([], doc, warning);
      }

      const parsedLines = this.parser.parseDocument(doc);
      const results: EvaluationResult[] = [];
      let consecutiveErrors = 0;
      let totalErrors = 0;
      let warning: string | undefined;

      for (const parsedLine of parsedLines) {
        const result = this.evaluator.evaluateLine(parsedLine);
        results.push(result);

        if (result.isError) {
          consecutiveErrors++;
          totalErrors++;
          if (consecutiveErrors >= MAX_CONSECUTIVE_ERRORS) {
            warning = `Stopped after ${MAX_CONSECUTIVE_ERRORS} consecutive errors.`;
            break;
          }
          if (totalErrors >= CALC_MAX_TOTAL_ERRORS) {
            warning = `Stopped after ${CALC_MAX_TOTAL_ERRORS} total errors.`;
            break;
          }
        } else if (!parsedLine.isComment && result.result !== null) {
          consecutiveErrors = 0;
        }
      }

      return this.builder.buildDecorations(results, doc, warning);
    }
  },
  { decorations: (v) => v.decorations },
);

// ---- Theme ----

const calcTheme = EditorView.baseTheme({
  '.cm-calc-result': {
    color: '#6b7280',
    fontStyle: 'italic',
    marginLeft: '1em',
    userSelect: 'none',
  },
  '.dark .cm-calc-result': {
    color: '#9ca3af',
  },
  '.cm-calc-error-line .cm-gutterElement': {
    color: '#ef4444 !important',
  },
  '.dark .cm-calc-error-line .cm-gutterElement': {
    color: '#f87171 !important',
  },
  '.cm-calc-warning': {
    display: 'inline-block',
    backgroundColor: '#fef3c7',
    color: '#92400e',
    padding: '4px 8px',
    borderRadius: '4px',
    fontFamily: 'system-ui, sans-serif',
    fontSize: '13px',
    marginBottom: '4px',
  },
  '.dark .cm-calc-warning': {
    backgroundColor: '#78350f',
    color: '#fef3c7',
  },
});

// ---- State ----

let isDark = false;
let currentFontSize = 15;
let editorView: EditorView | null = null;

// ---- Theme application ----

function applyTheme(dark: boolean): void {
  isDark = dark;
  document.documentElement.classList.toggle('dark', dark);
}

// ---- Editor creation ----

function createEditor(content: string, dark: boolean): void {
  applyTheme(dark);

  const container = document.getElementById('editor')!;
  container.innerHTML = '';

  editorView = new EditorView({
    parent: container,
    state: EditorState.create({
      doc: content,
      extensions: [
        new LanguageSupport(calcLanguage),
        syntaxHighlighting(dark ? calcHighlightStyleDark : calcHighlightStyleLight),
        calcPlugin,
        calcTheme,
        history(),
        keymap.of([...defaultKeymap, ...historyKeymap]),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            postToSwift({ type: 'contentChanged', content: update.state.doc.toString() });
          }
        }),
        EditorView.theme({
          '&': {
            height: '100%',
            fontSize: `${currentFontSize}px`,
          },
          '.cm-scroller': {
            fontFamily: "'SF Mono', Menlo, monospace",
            padding: '12px',
          },
          '.cm-content': {
            caretColor: dark ? '#e5e5e7' : '#1c1c1e',
          },
          '.cm-cursor': {
            borderLeftColor: dark ? '#e5e5e7' : '#1c1c1e',
          },
          '&.cm-focused .cm-selectionBackground, .cm-selectionBackground': {
            backgroundColor: dark ? '#264f78' : '#add6ff',
          },
        }),
      ],
    }),
  });
}

// ---- Bridge API ----

window.bridge = {
  setContent(content: string, dark: boolean): void {
    if (!editorView) {
      createEditor(content, dark);
      return;
    }

    applyTheme(dark);

    // Replace content if it differs from current
    const currentContent = editorView.state.doc.toString();
    if (currentContent !== content) {
      editorView.dispatch({
        changes: { from: 0, to: editorView.state.doc.length, insert: content },
      });
    }
  },

  setTheme(dark: boolean): void {
    applyTheme(dark);
    // Recreate editor to apply new syntax highlighting theme
    if (editorView) {
      const content = editorView.state.doc.toString();
      createEditor(content, dark);
    }
  },

  setFontSize(px: number): void {
    currentFontSize = px;
    // Recreate editor to apply new font size
    if (editorView) {
      const content = editorView.state.doc.toString();
      createEditor(content, isDark);
    }
  },

  resolveAttachment(): void {
    // Not used by calc editor
  },
};

// ---- Styles ----

const style = document.createElement('style');
style.textContent = `
  * { box-sizing: border-box; margin: 0; padding: 0; }
  html, body, #editor {
    height: 100%;
    width: 100%;
  }
  :root {
    --bg: #ffffff;
    --fg: #1c1c1e;
  }
  .dark {
    --bg: #1c1c1e;
    --fg: #e5e5e7;
  }
  body {
    font-family: -apple-system, BlinkMacSystemFont, sans-serif;
    color: var(--fg);
    background-color: var(--bg);
    -webkit-text-size-adjust: 100%;
  }
  .cm-editor {
    background: var(--bg);
  }
  .cm-gutters {
    background: var(--bg);
    border-right: none;
  }
`;
document.head.appendChild(style);

// ---- Init ----

postToSwift({ type: 'ready' });
