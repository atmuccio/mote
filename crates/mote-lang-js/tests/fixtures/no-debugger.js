function total(items) {
  debugger; // expect: no-debugger
  return items.length;
}
if (window.debug) debugger; // expect: no-debugger
const keyword = 'debugger';
// debugger inside a comment is fine
const obj = { debugger: true };
label: {
  debugger; // expect: no-debugger
}
