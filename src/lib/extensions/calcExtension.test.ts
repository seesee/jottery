import { describe, test, expect, beforeEach } from 'vitest';
import { CalcParser, CalcEvaluator } from './calcExtension';
import { Text } from '@codemirror/state';

describe('CalcParser', () => {
	const parser = new CalcParser();

	test('identifies comments', () => {
		expect(parser.isComment('# This is a comment')).toBe(true);
		expect(parser.isComment('  # Indented comment')).toBe(true);
		expect(parser.isComment('2 + 2')).toBe(false);
		expect(parser.isComment('x = 10')).toBe(false);
	});

	test('parses assignments', () => {
		const result1 = parser.parseAssignment('x = 10');
		expect(result1).toEqual({ variable: 'x', expression: '10' });

		const result2 = parser.parseAssignment('  myVar = 2 + 2  ');
		expect(result2).toEqual({ variable: 'myVar', expression: '2 + 2  ' });

		const result3 = parser.parseAssignment('_test123 = sin(pi/2)');
		expect(result3).toEqual({ variable: '_test123', expression: 'sin(pi/2)' });
	});

	test('ignores invalid assignments', () => {
		expect(parser.parseAssignment('2 + 2')).toBeNull();
		expect(parser.parseAssignment('x + y')).toBeNull();
		expect(parser.parseAssignment('123var = 10')).toBeNull(); // Invalid identifier
	});

	test('parses document with mixed content', () => {
		// Create a simple text document mock
		const lines = [
			'# Comment line',
			'x = 10',
			'y = 20',
			'x + y',
			'',
			'sqrt(16)'
		];

		const doc = {
			lines: lines.length,
			line: (i: number) => ({
				text: lines[i - 1],
				to: 0
			})
		} as Text;

		const parsed = parser.parseDocument(doc);

		expect(parsed).toHaveLength(5); // Empty line skipped
		expect(parsed[0].isComment).toBe(true);
		expect(parsed[1].isAssignment).toBe(true);
		expect(parsed[1].variable).toBe('x');
		expect(parsed[2].isAssignment).toBe(true);
		expect(parsed[2].variable).toBe('y');
		expect(parsed[3].isAssignment).toBe(false);
		expect(parsed[3].expression).toBe('x + y');
		expect(parsed[4].expression).toBe('sqrt(16)');
	});
});

describe('CalcEvaluator', () => {
	let evaluator: CalcEvaluator;

	beforeEach(() => {
		evaluator = new CalcEvaluator();
		evaluator.reset();
	});

	test('evaluates basic arithmetic', () => {
		const line = {
			lineNumber: 1,
			expression: '2 + 2',
			isComment: false,
			isAssignment: false
		};
		const result = evaluator.evaluateLine(line);
		expect(result.result).toBe('4');
		expect(result.isError).toBe(false);
	});

	test('evaluates complex arithmetic', () => {
		const line = {
			lineNumber: 1,
			expression: '(3 + 4) * 5 / 2',
			isComment: false,
			isAssignment: false
		};
		const result = evaluator.evaluateLine(line);
		expect(result.result).toBe('17.5');
		expect(result.isError).toBe(false);
	});

	test('handles variables across lines', () => {
		const line1 = {
			lineNumber: 1,
			expression: 'x = 10',
			isComment: false,
			isAssignment: true,
			variable: 'x'
		};
		const line2 = {
			lineNumber: 2,
			expression: 'x + 5',
			isComment: false,
			isAssignment: false
		};

		const result1 = evaluator.evaluateLine(line1);
		expect(result1.result).toBe('10'); // Assignments show their value

		const result2 = evaluator.evaluateLine(line2);
		expect(result2.result).toBe('15');
		expect(result2.isError).toBe(false);
	});

	test('handles multiple variables', () => {
		const assignments = [
			{
				lineNumber: 1,
				expression: 'a = 10',
				isComment: false,
				isAssignment: true,
				variable: 'a'
			},
			{
				lineNumber: 2,
				expression: 'b = 20',
				isComment: false,
				isAssignment: true,
				variable: 'b'
			},
			{
				lineNumber: 3,
				expression: 'c = a + b',
				isComment: false,
				isAssignment: true,
				variable: 'c'
			}
		];

		assignments.forEach((line) => evaluator.evaluateLine(line));

		const line = {
			lineNumber: 4,
			expression: 'c * 2',
			isComment: false,
			isAssignment: false
		};

		const result = evaluator.evaluateLine(line);
		expect(result.result).toBe('60');
	});

	test('evaluates math functions', () => {
		const tests = [
			{ expr: 'sqrt(16)', expected: '4' },
			{ expr: 'abs(-5)', expected: '5' },
			{ expr: 'round(3.7)', expected: '4' },
			{ expr: 'floor(3.7)', expected: '3' },
			{ expr: 'ceil(3.2)', expected: '4' }
		];

		tests.forEach(({ expr, expected }) => {
			const line = {
				lineNumber: 1,
				expression: expr,
				isComment: false,
				isAssignment: false
			};
			const result = evaluator.evaluateLine(line);
			expect(result.result).toBe(expected);
			expect(result.isError).toBe(false);
		});
	});

	test('evaluates trigonometric functions', () => {
		const line = {
			lineNumber: 1,
			expression: 'sin(pi / 2)',
			isComment: false,
			isAssignment: false
		};
		const result = evaluator.evaluateLine(line);
		expect(result.result).toBe('1');
		expect(result.isError).toBe(false);
	});

	test('handles unit conversions', () => {
		const tests = [
			{ expr: '5 miles to km', expected: '8.04' },
			{ expr: '100 cm to inches', expected: '39.37' },
			{ expr: '1 hour to seconds', expected: '3600' }
		];

		tests.forEach(({ expr, expected }) => {
			const line = {
				lineNumber: 1,
				expression: expr,
				isComment: false,
				isAssignment: false
			};
			const result = evaluator.evaluateLine(line);
			expect(result.result).toContain(expected); // Check numeric part starts correctly
			expect(result.isError).toBe(false);
		});
	});

	test('skips comments', () => {
		const line = {
			lineNumber: 1,
			expression: '# This is a comment',
			isComment: true,
			isAssignment: false
		};
		const result = evaluator.evaluateLine(line);
		expect(result.result).toBeNull();
		expect(result.isError).toBe(false);
	});

	test('handles errors gracefully', () => {
		const line = {
			lineNumber: 1,
			expression: 'invalid_var',
			isComment: false,
			isAssignment: false
		};
		const result = evaluator.evaluateLine(line);
		expect(result.isError).toBe(true);
		expect(result.result).toContain('Error');
	});

	test('handles division by zero', () => {
		const line = {
			lineNumber: 1,
			expression: '1 / 0',
			isComment: false,
			isAssignment: false
		};
		const result = evaluator.evaluateLine(line);
		expect(result.result).toBe('Infinity');
		expect(result.isError).toBe(false);
	});

	test('handles syntax errors', () => {
		const line = {
			lineNumber: 1,
			expression: '2 +',
			isComment: false,
			isAssignment: false
		};
		const result = evaluator.evaluateLine(line);
		expect(result.isError).toBe(true);
		expect(result.result).toContain('Error');
	});

	test('truncates long error messages', () => {
		const line = {
			lineNumber: 1,
			expression: 'this_is_a_very_long_undefined_variable_name_that_will_produce_a_long_error_message',
			isComment: false,
			isAssignment: false
		};
		const result = evaluator.evaluateLine(line);
		expect(result.isError).toBe(true);
		expect(result.result!.length).toBeLessThanOrEqual(60); // "Error: " + 50 chars + "..."
	});

	test('resets scope correctly', () => {
		const line1 = {
			lineNumber: 1,
			expression: 'x = 100',
			isComment: false,
			isAssignment: true,
			variable: 'x'
		};

		evaluator.evaluateLine(line1);

		// Reset and try to use the variable
		evaluator.reset();

		const line2 = {
			lineNumber: 1,
			expression: 'x + 1',
			isComment: false,
			isAssignment: false
		};

		const result = evaluator.evaluateLine(line2);
		expect(result.isError).toBe(true); // x should be undefined after reset
	});

	test('formats decimal results correctly', () => {
		const tests = [
			{ expr: '10 / 3', shouldContain: '3.33' }, // Truncated decimals
			{ expr: '10 / 2', expected: '5' }, // Integer result
			{ expr: 'pi', shouldContain: '3.14' } // Pi constant
		];

		tests.forEach(({ expr, expected, shouldContain }) => {
			const line = {
				lineNumber: 1,
				expression: expr,
				isComment: false,
				isAssignment: false
			};
			const result = evaluator.evaluateLine(line);
			if (expected) {
				expect(result.result).toBe(expected);
			} else if (shouldContain) {
				expect(result.result).toContain(shouldContain);
			}
			expect(result.isError).toBe(false);
		});
	});
});
