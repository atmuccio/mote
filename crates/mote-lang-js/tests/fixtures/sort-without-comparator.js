const sizes = [10, 9, 100, -1];
sizes.sort(); // expect: sort-without-comparator

[3, 20, 100].sort(); // expect: sort-without-comparator
[1.5, 0.25].toSorted(); // expect: sort-without-comparator

function fromText(text) {
  const ids = text.split(',').map(Number).sort(); // expect: sort-without-comparator
  const widths = text.split(' ').map(parseFloat);
  widths.sort(); // expect: sort-without-comparator
  return [ids, widths];
}

// Compliant.
function ok(names, text) {
  sizes.sort((a, b) => a - b);
  [3, 20, 100].sort((a, b) => b - a);
  [3, 1, 2].sort(); // single digits sort the same either way
  names.sort();
  ['b', 'a'].sort();
  [1, 'a'].sort();
  [].sort();
  text.split(',').map(trim).sort();
  text.split(',').map((s) => Number(s)).sort(); // callback not recognised: silent
  new Float64Array(sizes).sort();
  let changing = [10, 2];
  changing = names;
  changing.sort();
  const { sort } = helpers;
  sort();
  return sizes.sort;
}

function shadowed(text) {
  const Number = (s) => s.trim();
  return text.split(',').map(Number).sort();
}
