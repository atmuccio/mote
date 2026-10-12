export function positives(line) {
  const always = /error|warning|/.test(line); // expect: no-empty-alternative
  const leading = /|error/.test(line); // expect: no-empty-alternative
  const doubled = /\.(png||jpg)$/.test(line); // expect: no-empty-alternative
  const topDoubled = /a||b/; // expect: no-empty-alternative
  return [always, leading, doubled, topDoubled];
}

// ---- compliant ----
export function negatives(line) {
  const plain = /error|warning/.test(line);
  const optionalLast = /colou(r|)/.test(line);
  const optionalFirst = /(|s)$/.test(line);
  const escaped = /a\|b/.test(line);
  const inClass = /[|]/.test(line);
  const quantified = /(png|jpg)?$/.test(line);
  const empty = new RegExp('');
  return [plain, optionalLast, optionalFirst, escaped, inClass, quantified, empty];
}
