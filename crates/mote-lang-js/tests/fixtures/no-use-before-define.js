// Uses in the temporal dead zone.
render(config); // expect: no-use-before-define
const config = loadConfig();

function run() {
  total += 1; // expect: no-use-before-define
  let total = 0;
  console.log(typeof later); // expect: no-use-before-define
  const later = 1;
  return total + later;
}

const size = size + 1; // expect: no-use-before-define
const { a, b = a } = options();

new Widget(); // expect: no-use-before-define
class Widget {}

function loop(items) {
  for (const item of items) {
    if (item.last) use(limit); // expect: no-use-before-define
  }
  const limit = 3;
  return limit;
}

// Compliant.
function show() {
  return formatter.format(1);
}
const formatter = new Intl.NumberFormat();
const handler = () => settings.value;
const settings = { value: 1 };
class Early {
  method() {
    return Late.create();
  }
  static field = () => Late;
}
class Late {
  static create() {
    return new Late();
  }
}
hoisted();
function hoisted() {
  return legacy;
}
var legacy = 1;
const self = {
  get me() {
    return self;
  },
};
export { run, loop, show, handler, Early, Late, self, size, b };
