/**
 * The mathjs instance for calc notes.
 *
 * A calc note evaluates as soon as it's opened, and a server can turn any note holding
 * such a line into one. Nothing here runs code: mathjs has no access to that. What it
 * can do is run a tab out of memory, or spin it, so this removes what changes state or
 * parses further expressions, as mathjs's own security guidance advises, and limits
 * what can allocate or loop. It's a limit on the obvious ways, not a sandbox. Only a
 * worker could stop an unbounded loop, and a worker wouldn't protect the tab's memory.
 */
import { all, create } from 'mathjs';

/** The most elements one matrix or range can hold */
export const MAX_CALC_ELEMENTS = 1000;

/** How many variables and functions one line can read, which bounds recursion */
const MAX_SCOPE_READS = 200_000;

const TOO_BIG = 'Too large for a calc note';

const math = create(all);

// Kept for this file. Expressions lose it below.
const evaluate = math.evaluate;

type MathFunction = (...args: unknown[]) => unknown;
const plainFunctions = math as unknown as Record<string, MathFunction>;
const expressionFunctions = (math as unknown as { expression: { mathWithTransform: Record<string, MathFunction> } })
  .expression.mathWithTransform;

function toNumbers(value: unknown): number[] {
  if (typeof value === 'number') return [value];
  if (math.isBigNumber(value) || math.isFraction(value)) return [Number(value)];
  if (math.isMatrix(value)) return toNumbers(value.valueOf());
  if (Array.isArray(value)) return value.flatMap(toNumbers);
  return [];
}

const product = (numbers: number[]) => numbers.reduce((total, n) => total * Math.abs(n), 1);

/** How many elements a value holds. Anything that isn't a collection counts as one. */
export function elementCount(value: unknown): number {
  if (!math.isMatrix(value) && !Array.isArray(value)) return 1;
  return product(math.size(value as number[]).valueOf() as number[]);
}

/** The size of the result of an operation on two values, which mathjs broadcasts */
function broadcastLength(args: unknown[]): number {
  const sizes = args
    .filter(arg => Array.isArray(arg) || math.isMatrix(arg))
    .map(arg => math.size(arg as number[]).valueOf() as number[]);
  if (sizes.length < 2) return 1;

  const dimensions = Math.max(...sizes.map(size => size.length));
  return product(Array.from({ length: dimensions }, (_, i) => Math.max(...sizes.map(size => size[i] ?? 1))));
}

function rangeLength(args: unknown[]): number {
  const [start, end, step = 1] = typeof args[0] === 'string'
    ? args[0].split(':').map(Number)
    : args.slice(0, 3).flatMap(toNumbers);

  // A string range is start:end or start:step:end
  const [from, to, by] = typeof args[0] === 'string' && args[0].split(':').length === 3
    ? [start, step, end]
    : [start, end, step];

  return Math.abs((to - from) / by);
}

/** A function that refuses, when `size` says the result would be too large */
function limited(name: string, size: (args: unknown[]) => number, maximum = MAX_CALC_ELEMENTS) {
  const check = (args: unknown[]) => {
    if (!(size(args) <= maximum)) throw new Error(TOO_BIG);
  };

  const plain = plainFunctions[name];
  const inExpressions = expressionFunctions[name];

  const wrapped = (...args: unknown[]) => {
    check(args);
    return plain(...args);
  };
  // Some functions differ inside an expression: 1:5 includes the 5, and range(1, 5) doesn't
  wrapped.transform = (...args: unknown[]) => {
    check(args);
    return inExpressions(...args);
  };
  return wrapped;
}

function disabled(name: string) {
  return () => {
    throw new Error(`${name} isn't available in a calc note`);
  };
}

const sizeArguments = (args: unknown[]) => product(args.flatMap(toNumbers));
const sizeArgument = (args: unknown[]) => (Array.isArray(args[0]) || math.isMatrix(args[0]) ? product(toNumbers(args[0])) : 1);
const bigNumberArgument = (args: unknown[]) => (math.isBigNumber(args[0]) ? Number(args[0]) : 0);

// Without a limit these loop for as long as their argument, or run out of memory
const LOOP_LIMIT = 100_000;

math.import({
  // What changes state, or evaluates anything further
  ...Object.fromEntries([
    'import', 'createUnit', 'reviver', 'evaluate', 'parse', 'parser', 'compile', 'chain',
    'simplify', 'simplifyConstant', 'simplifyCore', 'rationalize', 'derivative', 'resolve',
    'symbolicEqual', 'leafCount',
    // What grows far faster than its arguments
    'setPowerset', 'setCartesian', 'bellNumbers', 'stirlingS2', 'composition', 'catalan',
  ].map(name => [name, disabled(name)])),

  range: limited('range', rangeLength),
  ones: limited('ones', sizeArguments),
  zeros: limited('zeros', sizeArguments),
  identity: limited('identity', args => {
    const sizes = args.flatMap(toNumbers);
    return sizes.length === 1 ? sizes[0] ** 2 : product(sizes);
  }),
  random: limited('random', sizeArgument),
  randomInt: limited('randomInt', sizeArgument),
  resize: limited('resize', args => product(toNumbers(args[1]))),
  diag: limited('diag', args => elementCount(args[0]) ** 2),
  kron: limited('kron', args => elementCount(args[0]) * elementCount(args[1])),
  concat: limited('concat', args => args.reduce<number>((total, arg) => total + elementCount(arg), 0)),
  ...Object.fromEntries(
    ['add', 'subtract', 'multiply', 'divide', 'mod', 'dotMultiply', 'dotDivide', 'dotPow']
      .map(name => [name, limited(name, broadcastLength)])
  ),

  factorial: limited('factorial', bigNumberArgument, LOOP_LIMIT),
  gamma: limited('gamma', bigNumberArgument, LOOP_LIMIT),
  combinations: limited('combinations', args => Number(args[1]), LOOP_LIMIT),
  combinationsWithRep: limited('combinationsWithRep', args => Number(args[1]), LOOP_LIMIT),
  permutations: limited('permutations', args => Number(args[1] ?? args[0]), LOOP_LIMIT),
}, { override: true });

/**
 * Variables and functions for one note. A function a note defines can call itself, and
 * there's no way to know how long that will take, so each line has a budget of reads.
 */
export class CalcScope extends Map<string, unknown> {
  private reads = 0;

  startLine(): void {
    this.reads = 0;
  }

  override get(name: string): unknown {
    if (++this.reads > MAX_SCOPE_READS) {
      throw new Error(TOO_BIG);
    }
    return super.get(name);
  }
}

/** Evaluate one line. A result that's too large isn't kept, for a later line to build on. */
export function evaluateLine(expression: string, scope: CalcScope): unknown {
  scope.startLine();
  const result = evaluate(expression, scope);

  if (elementCount(result) > MAX_CALC_ELEMENTS) {
    for (const [name, value] of Map.prototype.entries.call(scope)) {
      if (value === result) scope.delete(name);
    }
    throw new Error(TOO_BIG);
  }

  return result;
}

export const formatValue = (value: unknown): string => math.format(value, { precision: 14 });
