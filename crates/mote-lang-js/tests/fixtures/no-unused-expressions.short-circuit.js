// mote-test: params allow_short_circuit=false
export function run(ready, start, cache, load, value, options) {
  ready && start(); // expect: no-unused-expressions
  cache || load(); // expect: no-unused-expressions
  options ?? load(); // expect: no-unused-expressions
  ready && (value = 1); // expect: no-unused-expressions
  ready && value; // expect: no-unused-expressions
  if (ready) start();
  ready ? start() : load(); // ternaries keep the default behavior
  const both = ready && start();
  const arrow = () => ready && start(); // an arrow's expression body is a returned value
  return [both, arrow];
}
