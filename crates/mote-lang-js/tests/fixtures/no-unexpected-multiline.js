function positives(a, b, items, line, tag) {
  const total = a + b
  (items || []).forEach(console.log) // expect: no-unexpected-multiline

  const parts = line.split(',')
  [a, b] = parts // expect: no-unexpected-multiline

  const message = tag
  `hello ${a}` // expect: no-unexpected-multiline

  let x = a
  // a comment in between does not end the statement
  (b) // expect: no-unexpected-multiline

  const y = (a + b)
  (items) // expect: no-unexpected-multiline

  const z = a /* multi
  line */ (b) // expect: no-unexpected-multiline

  return [total, parts, message, x, y, z];
}

// ---- compliant ----
function negatives(a, b, items, line, tag, fn) {
  const total = a + b;
  (items || []).forEach(console.log);

  const parts = line.split(',');
  [a, b] = parts;

  const message = tag`hello ${a}`;
  const text = `plain ${a}`;

  const call = fn(
    a,
    b
  );
  const curried = fn(a)(b);
  const index = items[
    0
  ];
  const opt = fn
    ?.(a);
  const optIndex = items
    ?.[0];
  const chained = items
    .map((x) => x * 2)
    .filter(Boolean);

  const iife = (function () {
    return 1;
  })
  ();
  const arrowIife = (() => 2)
  ();

  const q = a
  ;(b || fn)();

  // Continuation lines indented deeper than the line they continue are deliberate layout.
  const listener = fn(a, b)
    (items);
  const first = fn(items)
    .filter(Boolean)
    [0];
  const tagged = fn(tag)
    `text`;

  return [total, parts, message, text, call, curried, index, opt, optIndex, chained, iife, arrowIife, q, listener, first, tagged];
}

positives(1, 2, [], 'a,b', String.raw);
negatives(1, 2, [], 'a,b', String.raw, (v) => v);
