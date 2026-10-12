const DIGITS = /\d+/g;
function hasNumber(text) {
  return DIGITS.test(text); // expect: stateful-regex-test
}

const STICKY = new RegExp('a', 'y');
const matches = (s) => STICKY.test(s); // expect: stateful-regex-test

function loop(items) {
  const WORD = /\w+/g;
  let n = 0;
  for (const item of items) {
    if (WORD.test(item)) n++; // expect: stateful-regex-test
  }
  return n;
}

function twice(a, b) {
  const re = /x/g;
  return re.test(a) && re.test(b); // expect: stateful-regex-test, stateful-regex-test
}

// Compliant.
const INVALID = /[^a-z]/gi;
function sanitize(name) {
  if (INVALID.test(name)) {
    return name.replace(INVALID, '_');
  }
  return name;
}
const PLAIN = /\d+/;
function ok(text) {
  return PLAIN.test(text);
}
const RESET = /x/g;
function reset(text) {
  RESET.lastIndex = 0;
  return RESET.test(text);
}
function local(text) {
  const re = /x/g;
  return re.test(text);
}
function count(text) {
  const re = /x/g;
  let n = 0;
  while (re.test(text)) n++;
  return n;
}
function perIteration(items) {
  return items.filter((item) => {
    const re = /x/g;
    return re.test(item);
  });
}
let reassigned = /x/g;
function viaLet(t) {
  return reassigned.test(t);
}
export { sanitize, hasNumber, matches, loop, twice, ok, reset, local, count, perIteration, viaLet };
