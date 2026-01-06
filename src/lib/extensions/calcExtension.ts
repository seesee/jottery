import { ViewPlugin, Decoration, EditorView, WidgetType } from '@codemirror/view';
import type { ViewUpdate, DecorationSet } from '@codemirror/view';
import { Text } from '@codemirror/state';
import * as math from 'mathjs';

const RESULT_PREFIX = '  ';

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

// Decoration Builder: Create inline result widgets
class DecorationBuilder {
	buildDecorations(results: EvaluationResult[], doc: Text): DecorationSet {
		const decorations: any[] = [];

		for (const result of results) {
			if (result.result === null) continue;

			// Get line end position
			const line = doc.line(result.lineNumber);
			const pos = line.to;

			// Create widget decoration
			const widget = Decoration.widget({
				widget: new ResultWidget(result.result, result.isError),
				side: 1 // Position after cursor
			});

			decorations.push(widget.range(pos));
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

// Theme: Styling for result widgets
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
	'.cm-calc-error': {
		color: '#ef4444', // red-500
		fontStyle: 'italic',
		marginLeft: '1em',
		userSelect: 'none'
	},
	'.dark .cm-calc-error': {
		color: '#f87171' // red-400
	}
});

// Export main extension
export function calcExtension() {
	return [calcPlugin, calcTheme];
}

// Export classes for testing
export { CalcParser, CalcEvaluator };
