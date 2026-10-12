function resize(width, height) {
  return width * height;
}

const move = (row, column, speed) => [row, column, speed];

function caller(width, height, size, row, column) {
  resize(height, width); // expect: swapped-arguments
  resize(size.height, size.width); // expect: swapped-arguments
  move(column, row, 1); // expect: swapped-arguments
  move(1, column, row);

  // ---- compliant ----
  resize(width, height);
  resize(size.width, size.height);
  resize(height, height);
  resize(height);
  return 0;
}

function compare(a, b) {
  return a - b;
}
function sortDesc(list) {
  return list.sort((a, b) => compare(b, a));
}

function range(start, end) {
  return end - start;
}
function reversed(start, end) {
  return range(end, start);
}

function span(left, right) {
  return right - left;
}
function mirrored(left, right) {
  return span(right, left);
}

function gcd(big, small) {
  return small === 0 ? big : gcd(small, big % small);
}

function recursive(first2, second2) {
  return recursive(second2, first2);
}

let changing = function (width, height) {
  return width + height;
};
changing = null;
function useChanging(width, height) {
  return changing(height, width);
}

export { caller, sortDesc, reversed, mirrored, gcd, recursive, useChanging };

function shift(diff, sequence1, sequence2, range1, range2) {
  return [diff, sequence1, sequence2, range1, range2];
}
function mirror(diff, sequence1, sequence2, range1, range2) {
  return shift(diff.swap(), sequence2, sequence1, range2, range1);
}
function numbered(sequence1, sequence2) {
  return shift(null, sequence2, sequence1);
}
function place(width, height, depth) {
  return [width, height, depth];
}
function crossTwo(width, height, depth) {
  return place(height, width, depth); // expect: swapped-arguments
}
export { mirror, numbered, crossTwo };
