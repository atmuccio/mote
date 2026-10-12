// expect-next-line: no-multi-str
const a = 'The file could not \
  be saved';
const b = 'one ' +
  'two';
const c = `template \
continued`;
const d = 'escaped \\n stays';
// expect-next-line: no-multi-str
const e = "double \
quoted";
// expect-next-line: no-multi-str
function f() { return g("x \
y"); }
const h = 'plain';
