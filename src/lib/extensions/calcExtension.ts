import { ViewPlugin, Decoration, EditorView, WidgetType } from '@codemirror/view';
import type { ViewUpdate, DecorationSet } from '@codemirror/view';
import { Text, Range } from '@codemirror/state';
import { HighlightStyle, syntaxHighlighting, LanguageSupport } from '@codemirror/language';
import { tags as t } from '@lezer/highlight';
import { StreamLanguage } from '@codemirror/language';
import { CalcScope, evaluateLine, formatValue } from './calcMath';
import { get } from 'svelte/store';
import { _ } from 'svelte-i18n';
import { CALC_MAX_EVALUATION_MS, CALC_MAX_LINES, CALC_MAX_TOTAL_ERRORS } from '../constants';

const RESULT_PREFIX = '  ';

// Limits to prevent performance issues with large/non-calc documents
const MAX_CONSECUTIVE_ERRORS = 10;

// Math.js built-in functions and constants
const BUILTIN_FUNCTIONS = new Set([
	'sin', 'cos', 'tan', 'asin', 'acos', 'atan', 'atan2',
	'sinh', 'cosh', 'tanh', 'asinh', 'acosh', 'atanh',
	'sqrt', 'cbrt', 'abs', 'ceil', 'floor', 'round', 'sign',
	'exp', 'log', 'log10', 'log2', 'ln',
	'min', 'max', 'mean', 'median', 'sum', 'prod',
	'random', 'randomInt',
	'gcd', 'lcm', 'mod', 'xgcd',
	'factorial', 'gamma', 'combinations', 'permutations'
]);

const BUILTIN_CONSTANTS = new Set(['pi', 'e', 'tau', 'phi', 'i', 'true', 'false', 'null', 'Infinity', 'NaN']);

// Simple tokenizer for calc language
const calcLanguage = StreamLanguage.define({
	token(stream, _state) {
		// Comments
		if (stream.match(/^#.*/)) {
			return 'comment';
		}

		// Numbers (including decimals and scientific notation)
		if (stream.match(/^[0-9]+\.?[0-9]*([eE][+-]?[0-9]+)?/)) {
			return 'number';
		}

		// Operators and punctuation
		if (stream.match(/^[+\-*/%^=()[\],]/)) {
			return 'operator';
		}

		// Words (functions, constants, variables, units)
		// Support Unicode identifiers (letters from any language) for international variable names
		const wordMatch = stream.match(/^[\p{L}_][\p{L}\p{N}_]*/u);
		if (wordMatch && typeof wordMatch !== 'boolean') {
			const word = wordMatch[0];
			if (BUILTIN_FUNCTIONS.has(word)) {
				return 'keyword';
			}
			if (BUILTIN_CONSTANTS.has(word)) {
				return 'atom';
			}
			return 'variableName';
		}

		// Skip whitespace
		if (stream.match(/^\s+/)) {
			return null;
		}

		// Default: consume one character
		stream.next();
		return null;
	}
});

// Syntax highlighting theme (dark mode - VS Code colors)
const calcHighlightStyleDark = HighlightStyle.define([
	{ tag: t.comment, color: '#6a9955' }, // Green
	{ tag: t.number, color: '#b5cea8' }, // Light green
	{ tag: t.keyword, color: '#4ec9b0' }, // Cyan (built-in functions)
	{ tag: t.atom, color: '#569cd6' }, // Blue (constants)
	{ tag: t.variableName, color: '#9cdcfe' }, // Light blue (user variables)
	{ tag: t.operator, color: '#d4d4d4' } // Light gray
]);

// Syntax highlighting theme (light mode - high contrast)
const calcHighlightStyleLight = HighlightStyle.define([
	{ tag: t.comment, color: '#008000' }, // Dark green
	{ tag: t.number, color: '#098658' }, // Dark green-blue
	{ tag: t.keyword, color: '#0070c1' }, // Dark blue (built-in functions)
	{ tag: t.atom, color: '#0000ff' }, // Blue (constants)
	{ tag: t.variableName, color: '#001080' }, // Dark blue (user variables)
	{ tag: t.operator, color: '#000000' } // Black
]);

// Line parsing result
interface ParsedLine {
	lineNumber: number;
	expression: string;
	isComment: boolean;
	isAssignment: boolean;
	variable?: string;
}

// Evaluation result
interface EvaluationResult {
	lineNumber: number;
	result: string | null;
	isError: boolean;
}

// Parser: Extract expressions from document
class CalcParser {
	parseDocument(doc: Text): ParsedLine[] {
		const lines: ParsedLine[] = [];

		for (let i = 1; i <= doc.lines; i++) {
			const line = doc.line(i);
			const text = line.text;

			// Skip empty lines
			if (!text.trim()) {
				continue;
			}

			// Check for comment
			if (this.isComment(text)) {
				lines.push({
					lineNumber: i,
					expression: text,
					isComment: true,
					isAssignment: false
				});
				continue;
			}

			// Check for assignment
			const assignment = this.parseAssignment(text);
			if (assignment) {
				lines.push({
					lineNumber: i,
					expression: text,
					isComment: false,
					isAssignment: true,
					variable: assignment.variable
				});
				continue;
			}

			// Regular expression
			lines.push({
				lineNumber: i,
				expression: text,
				isComment: false,
				isAssignment: false
			});
		}

		return lines;
	}

	isComment(text: string): boolean {
		return text.trimStart().startsWith('#');
	}

	parseAssignment(text: string): { variable: string; expression: string } | null {
		// Match pattern: identifier = expression
		// Support Unicode identifiers (letters from any language) using Unicode property escapes
		const match = text.match(/^\s*([\p{L}_][\p{L}\p{N}_]*)\s*=\s*(.+)$/u);
		if (match) {
			return {
				variable: match[1],
				expression: match[2]
			};
		}
		return null;
	}
}

// Evaluator: Process expressions with mathjs
class CalcEvaluator {
	private scope = new CalcScope();
	// Map Unicode variable names to ASCII internal names (math.js only supports ASCII)
	private unicodeToAscii: Map<string, string> = new Map();
	private varCounter = 0;

	reset() {
		this.scope = new CalcScope();
		this.unicodeToAscii.clear();
		this.varCounter = 0;
	}

	// Check if a variable name contains non-ASCII characters
	private isUnicodeVar(name: string): boolean {
		return /[^\x00-\x7F]/.test(name);
	}

	// Get or create an ASCII alias for a Unicode variable name
	private getAsciiAlias(unicodeName: string): string {
		if (!this.unicodeToAscii.has(unicodeName)) {
			this.unicodeToAscii.set(unicodeName, `_uvar${this.varCounter++}`);
		}
		return this.unicodeToAscii.get(unicodeName)!;
	}

	// Rewrite an expression, replacing Unicode variable names with ASCII aliases
	private rewriteExpression(expr: string): string {
		// Match Unicode identifiers: starts with letter or underscore, followed by letters/numbers/underscores
		// This regex matches any word that contains non-ASCII characters
		return expr.replace(/[\p{L}_][\p{L}\p{N}_]*/gu, (match) => {
			if (this.isUnicodeVar(match)) {
				return this.getAsciiAlias(match);
			}
			return match;
		});
	}

	evaluateLine(parsedLine: ParsedLine): EvaluationResult {
		// Skip comments
		if (parsedLine.isComment) {
			return {
				lineNumber: parsedLine.lineNumber,
				result: null,
				isError: false
			};
		}

		try {
			// Rewrite expression to replace Unicode variable names with ASCII aliases
			const rewrittenExpr = this.rewriteExpression(parsedLine.expression);
			const result = evaluateLine(rewrittenExpr, this.scope);

			// Format and return result (including for assignments)
			return {
				lineNumber: parsedLine.lineNumber,
				result: this.formatResult(result),
				isError: false
			};
		} catch (error: unknown) {
			return {
				lineNumber: parsedLine.lineNumber,
				result: this.formatError(error),
				isError: true
			};
		}
	}

	formatResult(value: unknown): string {
		// Handle different types
		if (typeof value === 'number') {
			// Limit decimal places
			return Number.isInteger(value)
				? value.toString()
				: value.toFixed(6).replace(/\.?0+$/, '');
		}

		// Use mathjs formatter for units, complex numbers, etc.
		return formatValue(value);
	}

	formatError(error: unknown): string {
		const message = error instanceof Error ? error.message : String(error);
		// Truncate long errors
		return `Error: ${message.substring(0, 50)}${message.length > 50 ? '...' : ''}`;
	}
}

/**
 * Evaluate a document in order, top to bottom, so that variables carry down, stopping
 * if it clearly isn't calculations or it's taking too long. A note evaluates as it
 * opens, so a slow one must not be able to hold the tab for long, line after line.
 */
export function evaluateDocument(
	evaluator: CalcEvaluator,
	parsedLines: ParsedLine[],
	now: () => number = () => performance.now()
): { results: EvaluationResult[]; warning?: { key: 'consecutiveErrors' | 'totalErrors' | 'tooSlow'; count?: number } } {
	const startedAt = now();
	const results: EvaluationResult[] = [];
	let consecutiveErrors = 0;
	let totalErrors = 0;

	for (const parsedLine of parsedLines) {
		if (now() - startedAt > CALC_MAX_EVALUATION_MS) {
			return { results, warning: { key: 'tooSlow' } };
		}

		const result = evaluator.evaluateLine(parsedLine);
		results.push(result);

		if (result.isError) {
			consecutiveErrors++;
			totalErrors++;

			if (consecutiveErrors >= MAX_CONSECUTIVE_ERRORS) {
				return { results, warning: { key: 'consecutiveErrors', count: MAX_CONSECUTIVE_ERRORS } };
			}
			if (totalErrors >= CALC_MAX_TOTAL_ERRORS) {
				return { results, warning: { key: 'totalErrors', count: CALC_MAX_TOTAL_ERRORS } };
			}
		} else if (!parsedLine.isComment && result.result !== null) {
			// Reset consecutive errors on successful evaluation
			consecutiveErrors = 0;
		}
	}

	return { results };
}

// Widget: Visual representation of result
class ResultWidget extends WidgetType {
	constructor(
		private result: string,
		private isError: boolean
	) {
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

// Widget: Warning message when limits are hit (inline at start of line)
class WarningWidget extends WidgetType {
	constructor(private message: string) {
		super();
	}

	toDOM(): HTMLElement {
		const span = document.createElement('span');
		span.className = 'cm-calc-warning';
		span.textContent = '⚠ ' + this.message + ' ';
		return span;
	}

	eq(other: WarningWidget): boolean {
		return other.message === this.message;
	}
}

// Decoration Builder: Create inline result widgets and error line markers
class DecorationBuilder {
	buildDecorations(results: EvaluationResult[], doc: Text, warning?: string): DecorationSet {
		const decorations: Range<Decoration>[] = [];

		// Add warning widget at the start of line 1 if present
		if (warning && doc.lines >= 1) {
			const warningWidget = Decoration.widget({
				widget: new WarningWidget(warning),
				side: -1 // Position before content
			});
			decorations.push(warningWidget.range(0));
		}

		for (const result of results) {
			if (result.result === null) continue;

			// Get line position
			const line = doc.line(result.lineNumber);

			if (result.isError) {
				// For errors, just mark the line (red gutter)
				const lineDeco = Decoration.line({
					class: 'cm-calc-error-line'
				});
				decorations.push(lineDeco.range(line.from));
			} else {
				// For successful results, show the value
				const widget = Decoration.widget({
					widget: new ResultWidget(result.result, result.isError),
					side: 1 // Position after cursor
				});
				decorations.push(widget.range(line.to));
			}
		}

		return Decoration.set(decorations, true);
	}
}

// ViewPlugin: Main orchestrator
const calcPlugin = ViewPlugin.fromClass(
	class {
		decorations: DecorationSet;
		private parser: CalcParser;
		private evaluator: CalcEvaluator;
		private builder: DecorationBuilder;

		constructor(view: EditorView) {
			this.parser = new CalcParser();
			this.evaluator = new CalcEvaluator();
			this.builder = new DecorationBuilder();
			this.decorations = this.compute(view);
		}

		update(update: ViewUpdate) {
			if (!update.docChanged) {
				return;
			}

			// Always recompute to keep decorations aligned
			// Expression evaluation is fast enough that we don't need to cache
			this.decorations = this.compute(update.view);
		}

		compute(view: EditorView): DecorationSet {
			// Reset evaluator for fresh scope
			this.evaluator.reset();

			const doc = view.state.doc;
			const totalLines = doc.lines;

			// Check if document is too long for calc mode
			if (totalLines > CALC_MAX_LINES) {
				const warning = get(_)('editor.calc.tooLong', { values: { lines: totalLines, max: CALC_MAX_LINES } });
				return this.builder.buildDecorations([], doc, warning);
			}

			// Parse all lines (gets current line numbers)
			const parsedLines = this.parser.parseDocument(doc);

			const { results, warning } = evaluateDocument(this.evaluator, parsedLines);

			// Build decorations with current line positions
			return this.builder.buildDecorations(
				results,
				doc,
				warning && get(_)(`editor.calc.${warning.key}`, { values: { count: warning.count } })
			);
		}

		destroy() {
			// Cleanup if needed
		}
	},
	{
		decorations: (v) => v.decorations
	}
);

// Theme: Styling for result widgets, error lines, and warnings
const calcTheme = EditorView.baseTheme({
	'.cm-calc-result': {
		color: '#6b7280', // gray-500
		fontStyle: 'italic',
		marginLeft: '1em',
		userSelect: 'none'
	},
	'.dark .cm-calc-result': {
		color: '#9ca3af' // gray-400
	},
	'.cm-calc-error-line .cm-gutterElement': {
		color: '#ef4444 !important' // red-500
	},
	'.dark .cm-calc-error-line .cm-gutterElement': {
		color: '#f87171 !important' // red-400
	},
	'.cm-calc-warning': {
		display: 'inline-block',
		backgroundColor: '#fef3c7', // amber-100
		color: '#92400e', // amber-800
		padding: '4px 8px',
		borderRadius: '4px',
		fontFamily: 'system-ui, sans-serif',
		fontSize: '13px',
		marginBottom: '4px'
	},
	'.dark .cm-calc-warning': {
		backgroundColor: '#78350f', // amber-900
		color: '#fef3c7' // amber-100
	}
});

// Export main extension
export function calcExtension(isDark: boolean = false) {
	return [
		new LanguageSupport(calcLanguage),
		syntaxHighlighting(isDark ? calcHighlightStyleDark : calcHighlightStyleLight),
		calcPlugin,
		calcTheme
	];
}

// Export classes for testing
export { CalcParser, CalcEvaluator };
