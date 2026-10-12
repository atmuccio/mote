import { poll } from './poll.js';

export const ONE_DAY_MS = 24 * 60 * 60 * 1000;
export const SIZES = [12, 14, 16];
const TABLE = { small: 8, large: [32, 64] };
const RATE = -0.075;

export function schedule(retries, list) {
  if (retries > 5) { // expect: no-magic-numbers
    setTimeout(poll, 86400000); // expect: no-magic-numbers
  }
  const scaled = retries * 3.5; // expect: no-magic-numbers
  const offset = -42 + retries; // expect: no-magic-numbers
  const port = 8080;
  let delay = 250;
  delay = 500;
  const first = list[3];
  const last = list[list.length - 1];
  const zero = list.indexOf(0) + 2;
  const hex = parseInt(retries, 16);
  const radix = Number.parseInt(retries, 36);
  const text = retries.toString(16);
  const opts = { timeout: 3000, sizes: [4, 5] };
  this.width = 640;
  switch (retries) {
    case 7: // expect: no-magic-numbers
      return list.slice(0, 10); // expect: no-magic-numbers
    default:
      return port + delay + first + last + zero + hex + radix + text + opts;
  }
}

export function withDefaults(size = 10, { depth = 3 } = {}) {
  return size + depth;
}

export const handler = () => {
  return compute(99); // expect: no-magic-numbers
};

export class Shape {
  static SIDES = 4;
  static TABLE = [3, 4, 5];
  static AREA = 2 * 3.14;
  radius = 7;
  grow() {
    this.radius *= 1.5; // expect: no-magic-numbers
  }
}

let counter = 12;
counter += 3; // expect: no-magic-numbers
poll(counter, 1, 2, -1, 0, 1.0, 0x1);

function local() {
  const table = [100, 200];
  const nested = [[3, 4], [5, 6]];
  return use([300, 400], table, nested); // expect: no-magic-numbers, no-magic-numbers
}

function tables(k, j, ref, useState) {
  const lookup = new Map([[k, 100], [j, 200]]);
  const allowed = new Set([404, 410]);
  for (const size of [12, 6, 3]) {
    use(size, lookup, allowed);
  }
  const state = { pageSize: ref(10) };
  const [count, setCount] = useState(5);
  use(new Map([[k, 7]])); // expect: no-magic-numbers
  use(Math.round(9.5)); // expect: no-magic-numbers
  return [state, count, setCount];
}
