import { ViewPlugin, Decoration, EditorView, WidgetType } from '@codemirror/view';
import type { ViewUpdate, DecorationSet } from '@codemirror/view';
import { Text } from '@codemirror/state';
import { HighlightStyle, syntaxHighlighting, LanguageSupport } from '@codemirror/language';
import { tags as t } from '@lezer/highlight';
import { StreamLanguage } from '@codemirror/language';
import * as math from 'mathjs';

const RESULT_PREFIX = '  ';

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
		const wordMatch = stream.match(/^[a-zA-Z_][a-zA-Z0-9_]*/);
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
		const match = text.match(/^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*(.+)$/);
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
	private scope: Record<string, any> = {};

	reset() {
		this.scope = {};
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
			const result = math.evaluate(parsedLine.expression, this.scope);

			// Format and return result (including for assignments)
			return {
				lineNumber: parsedLine.lineNumber,
				result: this.formatResult(result),
				isError: false
			};
		} catch (error: any) {
			return {
				lineNumber: parsedLine.lineNumber,
				result: this.formatError(error),
				isError: true
			};
		}
	}

	formatResult(value: any): string {
		// Handle different types
		if (typeof value === 'number') {
			// Limit decimal places
			return Number.isInteger(value)
				? value.toString()
				: value.toFixed(6).replace(/\.?0+$/, '');
		}

		// Use mathjs formatter for units, complex numbers, etc.
		return math.format(value, { precision: 14 });
	}

	formatError(error: any): string {
		const message = error.message || String(error);
		// Truncate long errors
		return `Error: ${message.substring(0, 50)}${message.length > 50 ? '...' : ''}`;
	}
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

// Decoration Builder: Create inline result widgets and error line markers
class DecorationBuilder {
	buildDecorations(results: EvaluationResult[], doc: Text): DecorationSet {
		const decorations: any[] = [];

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

			// Parse all lines (gets current line numbers)
			const parsedLines = this.parser.parseDocument(view.state.doc);

			// Evaluate in order (top-to-bottom) to maintain variable scope
			const results: EvaluationResult[] = [];
			for (const parsedLine of parsedLines) {
				const result = this.evaluator.evaluateLine(parsedLine);
				results.push(result);
			}

			// Build decorations with current line positions
			return this.builder.buildDecorations(results, view.state.doc);
		}

		destroy() {
			// Cleanup if needed
		}
	},
	{
		decorations: (v) => v.decorations
	}
);

// Theme: Styling for result widgets and error lines
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
