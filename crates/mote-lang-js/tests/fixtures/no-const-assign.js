// Writes to const bindings.
const limit = 10;
limit = 20; // expect: no-const-assign

function sum(items) {
  const total = 0;
  for (const item of items) {
    total += item.price; // expect: no-const-assign
  }
  return total;
}

const counter = 0;
counter++; // expect: no-const-assign

const [first, second] = [1, 2];
first = 3; // expect: no-const-assign
({ second } = { second: 4 }); // expect: no-const-assign

// Compliant.
const config = { debug: false };
config.debug = true;
const list = [];
list.push(1);
let mutable = 1;
mutable = 2;
for (const key in config) {
  console.log(key);
}
function shadow() {
  let limit = 1;
  limit = 2;
  return limit;
}
export { sum, shadow, mutable };
