const KEY = /(\w+)=/g;
function firstKey(line) {
  return KEY.exec(line)?.[1]; // expect: stateful-regex-test
}

const TOKEN = new RegExp('\w+', 'y');
export const next = (s) => TOKEN.exec(s); // expect: stateful-regex-test

// ---- compliant ----
const ALL = /\d+/g;
function all(text) {
  const out = [];
  let m;
  while ((m = ALL.exec(text)) !== null) out.push(m[0]);
  return out;
}

const DO = /\d+/g;
function doLoop(text) {
  let m;
  do {
    m = DO.exec(text);
    if (m) use(m);
  } while (m);
}

const BREAKING = /x/g;
function forever(text) {
  for (;;) {
    const m = BREAKING.exec(text);
    if (!m) break;
    use(m);
  }
}

const MANAGED = /x/g;
function managed(text, pos) {
  MANAGED.lastIndex = pos;
  return MANAGED.exec(text);
}

function local(text) {
  const re = /x/g;
  const a = re.exec(text);
  const b = re.exec(text);
  return [a, b];
}

const PLAIN = /x/;
function plain(text) {
  return PLAIN.exec(text);
}

export { firstKey, all, doLoop, forever, managed, local, plain };

const PRIMED = /\[(\w+)\]/g;
export function primed(key) {
  const out = [];
  let m = PRIMED.exec(key);
  while (m) {
    out.push(m[1]);
    m = PRIMED.exec(key);
  }
  return out;
}

export function tokenizer(input) {
  const re = /\w+/g;
  let match = re.exec(input);
  return {
    next: () => {
      const res = match;
      match = re.exec(input);
      return res;
    },
  };
}
