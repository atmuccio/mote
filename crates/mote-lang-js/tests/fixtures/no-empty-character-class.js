const tag = /<[]+>/; // expect: no-empty-character-class
const tail = /^a[]b$/g; // expect: no-empty-character-class
const bracket = /[]a]/; // expect: no-empty-character-class
// Compliant.
const any = /[^]/;
const escaped = /\[]/;
const inClass = /[\]a]/;
const inside = /[a[]/;
const normal = /[a-z]+/;
const str = '[]';
const re = new RegExp('[]');
const nested = /[[a]&&[b]]/v;
export { tag, tail, bracket, any, escaped, inClass, inside, normal, str, re, nested };
