export function parse(parts, codes, groups, rows) {
  const a = parts.map(parseInt); // expect: callback-extra-arguments
  const b = parts.map(Number.parseInt); // expect: callback-extra-arguments
  const c = parts.filter(parseInt); // expect: callback-extra-arguments
  const d = codes.map(String.fromCharCode); // expect: callback-extra-arguments
  const e = codes.map(String.fromCodePoint); // expect: callback-extra-arguments
  const f = groups.map(Array.from); // expect: callback-extra-arguments
  const g = Array.from(parts, parseInt); // expect: callback-extra-arguments
  const h = parts?.flatMap(parseInt); // expect: callback-extra-arguments
  const i = parts.some(parseInt, null); // expect: callback-extra-arguments
  const j = parts.find(parseInt); // expect: callback-extra-arguments
  const k = parts.findLastIndex(Number.parseInt); // expect: callback-extra-arguments
  return [a, b, c, d, e, f, g, h, i, j, k, rows];
}

// Compliant
export function ok(parts, values, set) {
  const a = parts.map((part) => parseInt(part, 10));
  const b = parts.map(Number);
  const c = parts.map(parseFloat);
  const d = parts.map(Number.parseFloat);
  const e = parts.filter(Boolean);
  const f = parts.map(String);
  const g = values.map(Math.abs);
  const h = parts.forEach(parseInt); // the result is not used
  const i = parts.map(JSON.parse);
  const j = Array.from(parts);
  const k = Array.from(parts, Number);
  const l = parts.map(encodeURIComponent);
  const m = parts.reduce(parseInt);
  const n = set.has(parseInt);
  const o = parts.map(parseInt.bind(null));
  const p = parts.map(parseInt, null, 'extra');
  return [a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p];
}

export function shadowed(parts) {
  const parseInt = (text) => Number(text);
  return parts.map(parseInt);
}

export function shadowedNumber(parts, Number) {
  return parts.map(Number.parseInt);
}

// Same-file functions are reported only with the local_functions parameter.
function format(value, digits = 2) {
  return value.toFixed(digits);
}
export const formatted = [1.5, 2.25].map(format);
