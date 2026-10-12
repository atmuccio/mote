const zw = 'a​b'; // expect: invisible-character
const nbsp = ' EUR'; // expect: invisible-character
const twice = ' x '; // expect: invisible-character
const bidi = 'user‮'; // expect: invisible-character
// comment with a bidi override ‮ inside // expect: invisible-character
// comment with a no-break space  is fine
/* comment with a zero width space​ is fine */
const ctl = 'ab'; // expect: invisible-character
const escaped = '10\u00A0EUR \u200B \u202E';
const accents = 'café naïve — ok';
const tpl = `x​${zw}y`; // expect: invisible-character
const re = /^﻿/; // expect: invisible-character
const family = '👨‍👩‍👧';
const persian = 'می‌خواهم';
const joiner = 'a‍b'; // expect: invisible-character
const a‍b = 1; // expect: invisible-character
let spaced = 2; // expect: invisible-character
const rlm = 'abc‏'; // expect: invisible-character
const mid = 'x﻿y'; // expect: invisible-character
const isolate = 'x'; // ⁦ isolate in a comment // expect: invisible-character
const nested = `a${'b​'}c`; // expect: invisible-character
const twoLiterals = [' ', ' ']; // expect: invisible-character, invisible-character
const esc = '\u001b[0m';
const alone = ' '; // expect: invisible-character
const trailing = `total: `; // expect: invisible-character
const nextToSpace = 'a  b'; // expect: invisible-character
const shyAlone = '­'; // expect: invisible-character
const unitSpace = 'a  b'; // expect: invisible-character
const del = ''; // expect: invisible-character

// Typography between visible characters of a literal: not reported.
const price = '10 EUR';
const narrow = '1 234';
const french = '« mode »';
const colon = `Total : ${price}`;
const shy = 'Donau­dampf';
const brand = 'VS Code';
const quoted = 'set `flip` or "x" y';
const edgeQuote = "it's "; // expect: invisible-character
const regexEdge = /a /; // expect: invisible-character
const regexMid = /a b/;
