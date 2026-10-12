const a = new RegExp('[a-z'); // expect: invalid-regexp
const b = new RegExp('a+', 'gg'); // expect: invalid-regexp
const c = RegExp('(?<name>x)(?<name>y)'); // expect: invalid-regexp
const d = new RegExp('*abc'); // expect: invalid-regexp
const e = new RegExp('abc', 'z'); // expect: invalid-regexp
const f = new RegExp(`(unclosed`); // expect: invalid-regexp
const g = new RegExp('\\p{Letter', 'u'); // expect: invalid-regexp
const h = new RegExp('a', 'uv'); // expect: invalid-regexp
const i = new RegExp(
  'a)', // expect: invalid-regexp
  'g',
);

// Compliant.
const ok1 = new RegExp('[a-z]+', 'gi');
const ok2 = new RegExp('\\d{3}-\\d{4}');
const ok3 = new RegExp(pattern);
const ok4 = new RegExp('^' + escape(name) + '$');
const ok5 = new RegExp('a', flags);
const ok6 = new RegExp(`${prefix}(`);
const ok7 = RegExp('(?<year>\\d{4})-(?<month>\\d{2})');
const ok8 = new RegExp('(?<x>a)|(?<x>b)'); // duplicate names in different alternatives
const ok9 = new RegExp('(?i:abc)def'); // modifiers
const ok10 = new RegExp('\\p{Script=Greek}', 'u');
const ok11 = new RegExp('[\\p{L}--[a-z]]', 'v');
const ok12 = new RegExp('');
const ok13 = new RegExp('a{', ''); // annex B: a lone brace is a literal
const ok14 = new RegExp('\\u{1F600}', 'u');
const ok15 = new RegExp('x', 'dgimsy');
const ok16 = new window.RegExp('[');
const ok17 = new RegExp(/[a-z]/, 'g');
const ok18 = new RegExp("it's");
const ok19 = new RegExp('\\/\\*[\\s\\S]*?\\*\\/|\\n', 'g');
const ok20 = new RegExp('[\\]\\[]');
const ok21 = new RegExp('\\d+\u00e9\x41');

function local(RegExp) {
  return new RegExp('[');
}

export { a, b, c, d, e, f, g, h, i, local };
