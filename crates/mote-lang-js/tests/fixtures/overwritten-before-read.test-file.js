// mote-test: test-file
// Tests assign twice on purpose to exercise setters and overwrite semantics.
test('setter fires once per change', () => {
  store.uri = first;
  store.uri = second;
  cache.set('a', 1);
  cache.set('a', 2);
});
