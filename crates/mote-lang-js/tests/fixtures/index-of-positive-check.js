function find(tags, text, list, item) {
  if (tags.indexOf('urgent') > 0) { // expect: index-of-positive-check
    alert('urgent');
  }
  const has = text.indexOf('--verbose') > 0; // expect: index-of-positive-check
  if (0 < list.indexOf(item)) {} // expect: index-of-positive-check
  if (list?.indexOf(item) > 0) {} // expect: index-of-positive-check
  if (text.indexOf('a') > 0) {} // expect: index-of-positive-check
  return has;
}

// Compliant.
function ok(tags, text, list, i) {
  if (tags.indexOf('urgent') !== -1) {}
  if (tags.indexOf('urgent') >= 0) {}
  if (tags.indexOf('urgent') > -1) {}
  if (text.indexOf('/', 1) > 0) {} // a start position makes the check meaningful
  if (text.indexOf(':') > 0) {} // a separator after a non-empty prefix
  if (text.indexOf('?') > 0) {}
  if (list.lastIndexOf(i) > 0) {}
  if (list.findIndex((x) => x > 1) > 0) {}
  if (tags.indexOf('urgent') > 1) {}
  if (tags.indexOf('urgent') >= 1) {}
  if (0 > tags.indexOf('urgent')) {}
  const index = tags.indexOf('urgent');
  if (index > 0) {} // a named variable: the intent is visible
  if (indexOf(tags) > 0) {}
  return tags.includes('urgent');
}
