function fix(date, text, price, list) {
  date.replace(/(\d+)-(\d+)/, '$2/$3'); // expect: invalid-replacement-reference
  text.replace(/(?:a|b)+/g, '[$1]'); // expect: invalid-replacement-reference
  text.replace(/(?<word>\w+)/, '<$<wrod>>'); // expect: invalid-replacement-reference
  text.replaceAll(/x/g, `$1`); // expect: invalid-replacement-reference
  text.replace(/([a-z])(?=\d)/, '$1$2'); // expect: invalid-replacement-reference
  text.replace(/[(](\w)/, '$2'); // expect: invalid-replacement-reference
  text?.replace(/a(b)/, '$9'); // expect: invalid-replacement-reference

  // ---- compliant ----
  date.replace(/(\d+)-(\d+)/, '$2/$1');
  price.replace(/\d+/, '$$&');
  price.replace(/\d+/, '$&.00');
  text.replace(/a/, "$`$'");
  text.replace(/(a)/, '$10');
  text.replace(/(a)(b)(c)(d)(e)(f)(g)(h)(i)(j)(k)/, '$11');
  text.replace(/(?<y>\d{4})-(?<m>\d\d)/, '$<m>/$<y>');
  text.replace(/\d+/, '$<name>');
  text.replace(/(?<!x)(a)/, '$1');
  text.replace(/\(\)(a)/, '$1');
  text.replace(re, '$5');
  text.replace(/x/, replacer);
  text.replace(/x/, `${prefix}$1`);
  text.replace('x', '$1');
  text.replace(/x/, '$0');
  text.replace(/[\]](a)/v, '$1');
  list.replace(/(a)/, '$');
  return text;
}
