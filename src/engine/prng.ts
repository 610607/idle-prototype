export function nextRandom(state: number): [number, number] {
  let t = (state + 0x6d2b79f5) | 0;
  const nextState = t;
  t = Math.imul(t ^ (t >>> 15), t | 1);
  t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
  const value = ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  return [value, nextState];
}

export function randomInt(
  state: number,
  minInclusive: number,
  maxInclusive: number,
): [number, number] {
  const [val, nextState] = nextRandom(state);
  const span = maxInclusive - minInclusive + 1;
  return [minInclusive + Math.floor(val * span), nextState];
}

export function pickWeighted<T extends { baseWeight: number }>(
  state: number,
  items: readonly T[],
): [T | null, number] {
  if (items.length === 0) return [null, state];
  const totalWeight = items.reduce((acc, item) => acc + Math.max(0, item.baseWeight), 0);
  if (totalWeight <= 0) return [items[0] ?? null, state];
  const [roll, nextState] = nextRandom(state);
  let threshold = roll * totalWeight;
  for (const item of items) {
    threshold -= Math.max(0, item.baseWeight);
    if (threshold <= 0) {
      return [item, nextState];
    }
  }
  return [items[items.length - 1] ?? null, nextState];
}

export function shuffleWithSeed<T>(state: number, items: readonly T[]): [T[], number] {
  const copy = [...items];
  let s = state;
  for (let i = copy.length - 1; i > 0; i--) {
    const [j, nextState] = randomInt(s, 0, i);
    s = nextState;
    const temp = copy[i];
    const swapVal = copy[j];
    if (temp !== undefined && swapVal !== undefined) {
      copy[i] = swapVal;
      copy[j] = temp;
    }
  }
  return [copy, s];
}
