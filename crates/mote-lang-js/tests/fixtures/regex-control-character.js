export function positives(text) {
  const ansi = /\[[0-9;]*m/g; // expect: regex-control-character
  const start = /^/; // expect: regex-control-character
  const inClass = /[,]/; // expect: regex-control-character
  return [ansi, start, inClass, text];
}

// ---- compliant ----
export function negatives(text) {
  const ansi = /\x1b\[[0-9;]*m/g;
  const control = /\cA|\x00/;
  const tab = /a	b/;
  const str = "[0m";
  return [ansi, control, tab, str, text];
}
