/**
 * A math.js instance that won't allocate more than a note can reasonably need.
 *
 * Notes arrive through sync, so what they ask for isn't under this device's control:
 * ones(60000, 60000) takes the web process down, and a note that does that every time
 * it opens can't be read here. The sizes are checked when the call runs, not by reading
 * the expression, so a size held in a variable counts too.
 *
 * This limits memory. A long calculation on arrays inside the limit can still take time.
 */

import { create, all } from 'mathjs';

export const math = create(all);

/** Far more than a note's calculation needs, and small enough to be harmless. */
const MAX_ELEMENTS = 1_000_000;

type Fn = ((...args: unknown[]) => unknown) & { transform?: Fn };

function toNumber(value: unknown): number {
  return typeof value === 'number' ? value : Number(value);
}

/** The dimensions in an array or matrix, as `[3, 4]` or `matrix([3, 4])`. */
function dimensions(value: unknown): number[] | null {
  if (Array.isArray(value)) return value.map(toNumber);
  if (value && typeof (value as { toArray?: unknown }).toArray === 'function') {
    return (value as { toArray: () => unknown[] }).toArray().map(toNumber);
  }
  return null;
}

function product(values: number[]): number {
  return values.reduce((total, value) => total * Math.abs(value), 1);
}

/** ones(3, 4), ones([3, 4]) and ones(3, 4, 'dense'). */
function bySizeArguments(args: unknown[]): number {
  const list = dimensions(args[0]);
  if (list) return product(list);
  return product(args.filter(arg => typeof arg !== 'string').map(toNumber));
}

/** identity(3) is 3 by 3, where ones(3) is three elements. */
function byIdentity(args: unknown[]): number {
  const sizes = dimensions(args[0]) ?? args.filter(arg => typeof arg !== 'string').map(toNumber);
  return sizes.length === 1 ? product(sizes) ** 2 : product(sizes);
}

/** random(size, min, max): a size is always an array, and random(1, 6) is a range. */
function byLeadingSize(args: unknown[]): number {
  const list = dimensions(args[0]);
  return list ? product(list) : 1;
}

/** range(0, 10), range(0, 10, 2), and the string form, range('0:2:10'). */
function byRange(args: unknown[]): number {
  let parts: number[];
  if (typeof args[0] === 'string') {
    const pieces = args[0].split(':').map(Number);
    parts = pieces.length === 3 ? [pieces[0], pieces[2], pieces[1]] : pieces;
  } else {
    parts = args.slice(0, 3).filter(arg => typeof arg !== 'boolean').map(toNumber);
  }
  const [start = 0, end = 0, step = 1] = parts;
  return step === 0 ? 0 : Math.abs((end - start) / step);
}

/** Holds as many elements as the first argument, squared. */
function bySquareOfFirst(args: unknown[]): number {
  const size = product(math.size(args[0] as number[]) as unknown as number[]);
  return size * size;
}

/** Holds as many elements as both arguments multiplied. */
function byBothArguments(args: unknown[]): number {
  return product(math.size(args[0] as number[]) as unknown as number[])
    * product(math.size(args[1] as number[]) as unknown as number[]);
}

function limited(fn: Fn, count: (args: unknown[]) => number): Fn {
  return (...args: unknown[]) => {
    const elements = count(args);
    if (!(elements <= MAX_ELEMENTS)) throw new Error('Too large to calculate here');
    return fn(...args);
  };
}

const limits: Record<string, (args: unknown[]) => number> = {
  ones: bySizeArguments,
  zeros: bySizeArguments,
  identity: byIdentity,
  random: byLeadingSize,
  randomInt: byLeadingSize,
  pickRandom: () => 1,
  range: byRange,
  resize: args => (dimensions(args[1]) ? product(dimensions(args[1])!) : 1),
  diag: bySquareOfFirst,
  kron: byBothArguments,
};

const unlimited = math as unknown as Record<string, Fn> & { expression: { transform: Record<string, Fn> } };
const replacements: Record<string, Fn> = {};

for (const [name, count] of Object.entries(limits)) {
  const replacement = limited(unlimited[name], count);
  // In an expression some functions go through a transform, and range(1, 3) means
  // something different there, so it has to stay what's called.
  const transform = unlimited.expression.transform[name];
  if (transform) replacement.transform = limited(transform, count);
  replacements[name] = replacement;
}

math.import(replacements, { override: true });
