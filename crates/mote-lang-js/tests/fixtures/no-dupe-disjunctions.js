export function positives(name, text) {
  const images = /\.(png|jpg|png)$/; // expect: no-dupe-disjunctions
  const lines = text.split(/\r|\r\n/); // expect: no-dupe-disjunctions
  const days = text.replace(/Mon|Monday/g, 'M'); // expect: no-dupe-disjunctions
  const nested = /(?:a|(b|c|b))/; // expect: no-dupe-disjunctions
  const top = text.match(/^a|^ab/); // expect: no-dupe-disjunctions
  const classes = /[a-z]|[a-z]\d/.exec(text); // expect: no-dupe-disjunctions
  const emoji = text.replaceAll(/😀|😀x/gu, ''); // expect: no-dupe-disjunctions
  const same = /x|y|x/.test(text); // expect: no-dupe-disjunctions
  return [images, lines, days, nested, top, classes, emoji, same, name];
}

// ---- compliant ----
export function negatives(text) {
  const images = /\.(png|jpe?g)$/;
  const lines = text.split(/\r\n|\r|\n/);
  // Inside a group, backtracking can still pick the longer alternative.
  const grouped = text.match(/(a|ab)c/);
  // A quantified earlier alternative does not have a fixed match.
  const quantified = text.match(/a+|a+b/);
  // Different case is a different alternative (only the same text is a duplicate).
  const cased = /(a|A)/;
  const different = /foo|bar|baz/;
  const optional = /(a|)/;
  const unicode = text.split(/😀x|😀/u);
  const lookahead = /(?=a|b)x/;
  // A stored fragment may be spliced into a bigger pattern; `test` gives the same answer.
  const encoding = /u|u8|U|L/;
  const tested = /Mon|Monday/.test(text);
  const found = text.search(/\r|\r\n/);
  return [images, lines, grouped, quantified, cased, different, optional, unicode, lookahead, encoding, tested, found];
}
