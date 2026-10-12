export function positives(name) {
  const upper = /^\p{Lu}/.test(name); // expect: unicode-escape-needs-flag
  const notLetter = /\P{L}+/g; // expect: unicode-escape-needs-flag
  const script = /[\p{Script=Greek}]/; // expect: unicode-escape-needs-flag
  const codePoint = /\u{1F600}/; // expect: unicode-escape-needs-flag
  const repeated = /\u{41}/; // expect: unicode-escape-needs-flag
  return [upper, notLetter, script, codePoint, repeated];
}

// ---- compliant ----
export function negatives(name) {
  const upper = /^\p{Lu}/u.test(name);
  const sets = /[\p{L}--\p{Lu}]/v;
  const codePoint = /\u{1F600}/u;
  const escapedBackslash = /\\p{L}/;
  const plainP = /p{2}/;
  const repeatedP = /\p{2}/;
  const bmp = /\u0041/;
  const constructed = new RegExp('\\p{L}');
  return [upper, sets, codePoint, escapedBackslash, plainP, repeatedP, bmp, constructed];
}
