import { describe, test, expect, beforeEach } from 'vitest';
import { CalcParser, CalcEvaluator, evaluateDocument } from './calcExtension';
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

	test('parses Unicode variable names (Japanese)', () => {
		const result1 = parser.parseAssignment('元金 = 1000');
		expect(result1).toEqual({ variable: '元金', expression: '1000' });

		const result2 = parser.parseAssignment('利率 = 0.05');
		expect(result2).toEqual({ variable: '利率', expression: '0.05' });

		const result3 = parser.parseAssignment('年数 = 10');
		expect(result3).toEqual({ variable: '年数', expression: '10' });
	});

	test('parses Unicode variable names (other languages)', () => {
		// German
		const german = parser.parseAssignment('Zinssatz = 0.05');
		expect(german).toEqual({ variable: 'Zinssatz', expression: '0.05' });

		// Greek
		const greek = parser.parseAssignment('κεφάλαιο = 1000');
		expect(greek).toEqual({ variable: 'κεφάλαιο', expression: '1000' });

		// Russian
		const russian = parser.parseAssignment('капитал = 1000');
		expect(russian).toEqual({ variable: 'капитал', expression: '1000' });

		// Chinese
		const chinese = parser.parseAssignment('本金 = 1000');
		expect(chinese).toEqual({ variable: '本金', expression: '1000' });
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

	test('handles compound interest calculation with ASCII variables', () => {
		const lines = [
			{
				lineNumber: 1,
				expression: 'principal = 1000',
				isComment: false,
				isAssignment: true,
				variable: 'principal'
			},
			{
				lineNumber: 2,
				expression: 'rate = 0.05',
				isComment: false,
				isAssignment: true,
				variable: 'rate'
			},
			{
				lineNumber: 3,
				expression: 'years = 10',
				isComment: false,
				isAssignment: true,
				variable: 'years'
			},
			{
				lineNumber: 4,
				expression: 'principal * (1 + rate)^years',
				isComment: false,
				isAssignment: false
			}
		];

		// Evaluate all lines
		lines.forEach((line) => evaluator.evaluateLine(line));

		// The last line should calculate compound interest: 1000 * (1.05)^10 ≈ 1628.89
		const result = evaluator.evaluateLine(lines[3]);
		expect(result.isError).toBe(false);
		expect(result.result).toContain('1628');
	});

	test('handles Unicode variable names (Japanese compound interest)', () => {
		// Japanese variable names: 元金 (principal), 利率 (rate), 年数 (years)
		const lines = [
			{
				lineNumber: 1,
				expression: '元金 = 1000',
				isComment: false,
				isAssignment: true,
				variable: '元金'
			},
			{
				lineNumber: 2,
				expression: '利率 = 0.05',
				isComment: false,
				isAssignment: true,
				variable: '利率'
			},
			{
				lineNumber: 3,
				expression: '年数 = 10',
				isComment: false,
				isAssignment: true,
				variable: '年数'
			},
			{
				lineNumber: 4,
				expression: '元金 * (1 + 利率)^年数',
				isComment: false,
				isAssignment: false
			}
		];

		// Evaluate all lines in order
		lines.slice(0, 3).forEach((line) => evaluator.evaluateLine(line));

		// The last line should calculate compound interest: 1000 * (1.05)^10 ≈ 1628.89
		const result = evaluator.evaluateLine(lines[3]);
		expect(result.isError).toBe(false);
		expect(result.result).toContain('1628');
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

describe('what a calc note can reach', () => {
	let evaluator: CalcEvaluator;

	beforeEach(() => {
		evaluator = new CalcEvaluator();
	});

	const evaluate = (expression: string) =>
		evaluator.evaluateLine({ lineNumber: 1, expression, isComment: false, isEmpty: false } as never);

	test.each([
		['a huge matrix of ones', 'ones(30000, 30000)'],
		['a huge matrix of zeros', 'zeros(100000, 100000)'],
		['a huge range', 'sum(1:2000000000)'],
		['a huge range by name', 'range(1, 2000000000)'],
		['a huge identity matrix', 'identity(100000)'],
		['a huge random matrix', 'random([100000, 100000])'],
		['a huge kron product', 'kron(ones(1000), ones(1000))'],
		['a matrix built up by broadcast', 'ones(1000, 1) + ones(1, 1000)'],
		['an outer product', 'ones(1000, 1) * ones(1, 1000)'],
		['a power set', 'setPowerset(1:30)'],
		['a long factorial', 'factorial(bignumber(1000000000))'],
		['a long combination', 'combinations(1000000000, 500000000)'],
	])('refuses %s, and carries on', (_name, expression) => {
		const result = evaluate(expression);

		expect(result.isError).toBe(true);
		expect(evaluate('1 + 1').result).toBe('2');
	});

	test('stops a function that calls itself for ever', () => {
		evaluate('f(n) = n > 0 ? f(n - 1) + f(n - 1) : 1');

		expect(evaluate('f(60)').isError).toBe(true);
	});

	test('does not keep a value that was too large', () => {
		evaluate('a = (ones(1, 1000) == 1) == (ones(1000, 1) == 1)');

		expect(evaluate('a').isError).toBe(true);
	});

	test.each([
		['createUnit("knots")'], ['import({})'], ['evaluate("1+1")'], ['parse("1+1")'],
		['simplify("x+x")'], ['derivative("x^2", "x")'], ['resolve("x")'],
	])('refuses %s', expression => {
		expect(evaluate(expression).isError).toBe(true);
	});

	test('cannot change units for other notes', () => {
		evaluate('createUnit("wibble")');

		expect(new CalcEvaluator().evaluateLine({ lineNumber: 1, expression: '1 wibble', isComment: false, isEmpty: false } as never).isError).toBe(true);
	});

	test.each([
		['1:5', '[1, 2, 3, 4, 5]'],
		['sum(1:100)', '5050'],
		['mean([1, 2, 3])', '2'],
		['det([[1, 2], [3, 4]])', '-2'],
		['[1, 2] * [3, 4]', '11'],
		['[[1, 2], [3, 4]] * [[1, 0], [0, 1]]', '[[1, 2], [3, 4]]'],
		['[1, 2] + 1', '[2, 3]'],
		['[1, 2] .* [3, 4]', '[3, 8]'],
		['std([2, 4, 4, 4])', '1'],
		['5 cm to inch', '1.9685039370079 inch'],
		['ones(2, 2)', '[[1, 1], [1, 1]]'],
		['identity(2)', '[[1, 0], [0, 1]]'],
		['factorial(5)', '120'],
		['combinations(5, 2)', '10'],
		['concat([1, 2], [3])', '[1, 2, 3]'],
		['2 km + 500 m', '2.5 km'],
		['sqrt(16)', '4'],
	])('still works out %s', (expression, expected) => {
		expect(evaluate(expression).result).toBe(expected);
	});

	test('keeps functions a note defines', () => {
		evaluate('g(x) = x^2');

		expect(evaluate('g(3)').result).toBe('9');
	});
});

describe('evaluateDocument', () => {
	const line = (lineNumber: number, expression: string) =>
		({ lineNumber, expression, isComment: false, isEmpty: false }) as never;

	test('stops when a note takes too long, however many lines are left', () => {
		let time = 0;
		const clock = () => (time += 600);

		const { results, warning } = evaluateDocument(new CalcEvaluator(), [line(1, '1'), line(2, '2'), line(3, '3'), line(4, '4')], clock);

		expect(results.length).toBeLessThan(4);
		expect(warning?.key).toBe('tooSlow');
	});

	test('evaluates everything when there is time', () => {
		const { results, warning } = evaluateDocument(new CalcEvaluator(), [line(1, 'a = 2'), line(2, 'a * 3')]);

		expect(results.map(result => result.result)).toEqual(['2', '6']);
		expect(warning).toBeUndefined();
	});

	test('stops on a run of errors, which means this is not calculations', () => {
		const lines = Array.from({ length: 20 }, (_, index) => line(index + 1, 'not calculations at all'));

		const { warning } = evaluateDocument(new CalcEvaluator(), lines);

		expect(warning?.key).toBe('consecutiveErrors');
	});
});
