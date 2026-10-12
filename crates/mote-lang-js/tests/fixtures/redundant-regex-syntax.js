const a = /[0-9]{1}x/; // expect: redundant-regex-syntax
const b = /\d{1,1}/; // expect: redundant-regex-syntax
const c = /\d{1,}:\d{0,1}/; // expect: redundant-regex-syntax, redundant-regex-syntax
const d = /a{0,}/; // expect: redundant-regex-syntax
const e = /[a-z_x]/; // expect: redundant-regex-syntax
const f = /[aa]/; // expect: redundant-regex-syntax
const g = /[\d0-9]/; // expect: redundant-regex-syntax
const h = /[\w_]+/; // expect: redundant-regex-syntax
const i = /[\w\d]/; // expect: redundant-regex-syntax
const j = /[a]bc/; // expect: redundant-regex-syntax
const j2 = /\w+[_]\d/; // expect: redundant-regex-syntax
const k = /a   b/; // expect: redundant-regex-syntax
const l = /x{1}?/; // expect: redundant-regex-syntax
const m = /([ab]{1})/; // expect: redundant-regex-syntax

// Compliant.
const n = /\d{2}/;
const o = /\d{1,3}/;
const p = /\d{2,}/;
const q = /[.]/;
const r = /[ ]/;
const s = /[*]/;
const t = /[a-zA-Z0-9_-]/;
const u = /[^a]/;
const v = /a b/;
const w = /a {3}b/;
const x = /[\s\S]/;
const y = /[aA]/i;
const z = /[\p{L}\p{N}]/u;
const aa = new RegExp('[aa]');
const bb = /a?/;
const cc = /[-a-]/; // expect: redundant-regex-syntax
const dd = /[abc|def|ghi]/; // expect: redundant-regex-syntax
const ee = /[✍️⚠️]/;
const g2 = /[\spunctuation]/;

// Empty non-capturing groups.
const eg1 = /a(?:)b/; // expect: redundant-regex-syntax
const eg2 = /x(?:|)y/; // expect: redundant-regex-syntax
const eg3 = /(?:)*z/; // expect: redundant-regex-syntax
const eg4 = /(a(?:))+/; // expect: redundant-regex-syntax
const eg5 = /(?:)/;
const eg6 = /(a)\1(?:)0/;
// Empty capturing groups keep a group number (padding for $10) or record a position.
const eg7 = /(hi)()()()()()()()()()/;
const eg8 = /(?<tail>)$/;
const eg9 = /a()b/d;
const eg10 = /x(|)y/;
const eg11 = /()/;
const eg12 = /(a|)b/;
const eg13 = /(?=)a/;
const eg14 = /\(\)/;
const eg15 = /[()]/;
const eg16 = new RegExp('a(?:)b');

// Alternations of single characters.
const al1 = /\s*(,|;)\s*/; // expect: redundant-regex-syntax
const al2 = /a|b|c/; // expect: redundant-regex-syntax
const al3 = /x(?:\s|,|\.)+y/; // expect: redundant-regex-syntax
const al4 = /(?:\r|\n)$/; // expect: redundant-regex-syntax
const al5 = /(?=\d|-)/; // expect: redundant-regex-syntax
const al6 = /(?:é|e)/u; // expect: redundant-regex-syntax
const al7 = /\r\n|\r|\n/;
const al8 = /(a|a)/;
const al9 = /(a|A)/i;
const al10 = /(a|bc)/;
const al11 = /(.|\n)/;
const al12 = /(\p{L}|_)/u;
const al13 = /^a|b$/;
const al14 = /(a|)/;
const al15 = /(😀|x)/;
const al16 = /(\b|x)/;
const al17 = /(\1|x)(y)/;
const al18 = /([ab]|c)/;

// Group names nothing reads.
function names(line, s) {
  const n1 = /(?<year>\d{4})-\d\d/.test(line); // expect: redundant-regex-syntax
  const n2 = line.split(/(?<sep>[,;])/); // expect: redundant-regex-syntax
  const n3 = line.search(/(?<word>\w+)!/); // expect: redundant-regex-syntax
  const n4 = line.replace(/(?<num>\d+)/g, '#'); // expect: redundant-regex-syntax
  const n5 = line.replace(/(?<num>\d+)-(?<rest>x)/g, '<$<num>>'); // expect: redundant-regex-syntax
  const n6 = s?.split(/(?<sep>;)/); // expect: redundant-regex-syntax
  // Compliant: the name is or may be read.
  const r1 = /(?<year>\d{4})/.exec(line).groups.year;
  const r2 = line.match(/(?<year>\d{4})/);
  const r3 = line.replace(/(?<num>\d+)/g, '<$<num>>');
  const r4 = line.replace(/(?<num>\d+)/g, (...args) => args.at(-1).num);
  const r5 = /(?<q>['"]).*?\k<q>/.test(line);
  const r6 = [...line.matchAll(/(?<k>\w+)=/g)];
  const r7 = line.replace(/(?<num>\d+)/g, replacement);
  const DATE = /(?<year>\d{4})/;
  const r8 = DATE.test(line);
  const r9 = /(?<a>x)/.exec(line);
  const r10 = line.replace(/(?<num>\d+)/g, `$<num>`);
  return [n1, n2, n3, n4, n5, n6, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10];
}
