function clean(html, line, text) {
  const a = html.replace(/<!--.*?/g, ''); // expect: regex-lazy-quantifier-at-end
  const b = line.replace(/key=\w+?/, 'key=*'); // expect: regex-lazy-quantifier-at-end
  const c = line.split(/,\s*?/); // expect: regex-lazy-quantifier-at-end
  const d = /id:(\d{2,6}?)/.exec(line); // expect: regex-lazy-quantifier-at-end
  const e = line.replaceAll(/a|b+?/g, '-'); // expect: regex-lazy-quantifier-at-end
  const f = /^\s*#.*?/.test(line); // expect: regex-lazy-quantifier-at-end
  const g = text.match(/"(.+?)/); // expect: regex-lazy-quantifier-at-end
  const h = [...text.matchAll(/\[(?:[^\]]*?)/g)]; // expect: regex-lazy-quantifier-at-end
  const i = text.search(/x??/); // expect: regex-lazy-quantifier-at-end
  const j = text?.match(/<b>.*?/); // expect: regex-lazy-quantifier-at-end
  const k = text.split(/(-.+?)/, 3); // expect: regex-lazy-quantifier-at-end
  return [a, b, c, d, e, f, g, h, i, j, k];
}

// Compliant
const MEDIA = /\/media\/.*?/; // use not visible here
const built = new RegExp('<!--.*?');

function ok(html, line, items) {
  const a = html.replace(/<!--.*?-->/g, '');
  const b = line.replace(/x.*?$/, '');
  const c = line.replace(/a{2}?/, ''); // a fixed count
  const d = line.replace(/a+?\b/, '');
  const e = line.replace(/\?/, '');
  const f = line.replace(/.*/, '');
  const g = line.split(',.*?');
  const h = /(?=a+?)b/.test(line);
  const i = /(?:a.*?)+/.test(line); // a repeated group
  const j = /^(.*?)\s*$/m.exec(line);
  const k = MEDIA.test(line);
  const l = line.replace(/a.*?(?=;)/, '');
  const m = items.find(/x+?/); // not a string search method
  const n = /[+?]/.test(line);
  const o = /a\*?/.test(line);
  const p = /(?=x)??y/.test(line);
  const q = line.replace(/<p>(.*?)<\/p>/g, '$1');
  return [a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q, built];
}
