const code = /^[A-Z]{0}\d+$/; // expect: useless-regex-quantifier
const pair = /(?:ab){0,0}c/; // expect: useless-regex-quantifier
const word = /(?=\w)?\w+/; // expect: useless-regex-quantifier
const start = /(?:^)*abc/; // expect: useless-regex-quantifier
const edge = /(?:\b|$)?x/; // expect: useless-regex-quantifier
const neg = /(?!\d){0,2}\w/; // expect: useless-regex-quantifier
const nested = /((?:\b))?y/; // expect: useless-regex-quantifier
const inLook = /(?=a{0})b/; // expect: useless-regex-quantifier

// Compliant.
const range = /^[A-Z]{0,2}\d+$/;
const many = /a{0,}/;
const ten = /a{10}/;
const plus = /(?:\b)+x/;
const once = /(?=\w)\w+/;
const sep = /(?:^|,)?value/;
const opt = /(a)?b/;
const empty = /(?:)?c/;
const emptyCap = /()*d/;
const str = new RegExp('a{0}');
const cls = /[{0}]/;
const esc = /\{0\}/;
