const id = new Symbol('id'); // expect: no-new-native-nonconstructor
const big = new BigInt(10); // expect: no-new-native-nonconstructor

// Compliant.
const id2 = Symbol('id');
const big2 = BigInt(10);
const s = new String('x');
function shadowed() {
  class Symbol {}
  return new Symbol();
}
export { id, big, id2, big2, s, shadowed };
