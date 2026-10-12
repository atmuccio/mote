// mote-test: params local_functions=true
function format(value, digits = 2) {
  return value.toFixed(digits);
}
const pad = (text, width = 8) => text.padStart(width);
function render(item, index = 0) {
  return `${index}: ${item}`;
}
function save(item, options = {}) {
  return { item, options };
}
function load(item, i, list = []) {
  return list.length ? item : i;
}
function plain(item, other) {
  return item + other;
}
function variadic(item, size = 1) {
  return arguments.length > 1 ? item * size : item;
}
function ignored(item, _scale = 1) {
  return item * _scale;
}
let changed = (item, scale = 1) => item * scale;
changed = (item) => item;

export function use(values, texts) {
  const a = values.map(format); // expect: callback-extra-arguments
  const b = texts.map(pad); // expect: callback-extra-arguments
  values.forEach(save); // expect: callback-extra-arguments
  const c = values.map(render);
  const d = values.map(load);
  const e = values.map(plain);
  const f = values.map(variadic);
  const g = values.map(ignored);
  const h = values.map(changed);
  const i = values.map((v) => format(v));
  const j = Array.from(texts, pad); // expect: callback-extra-arguments
  const k = values.map(parseInt); // expect: callback-extra-arguments
  return [a, b, c, d, e, f, g, h, i, j, k];
}
