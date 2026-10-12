const flag = new Boolean(false); // expect: no-new-wrappers
const count = new Number(input); // expect: no-new-wrappers
const text = new String('a'); // expect: no-new-wrappers
const plain = Number(input);
const str = String(x);
const obj = new Object();
function local() {
  const String = class {};
  return new String();
}
const date = new Date();
const member = new window.Number(1);
