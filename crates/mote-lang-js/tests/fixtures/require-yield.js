function* ids(items) { // expect: require-yield
  for (const item of items) {
    return item.id;
  }
}

const gen = function* () { // expect: require-yield
  compute();
  return 1;
};

export default function* () { // expect: require-yield
  const inner = function* () {
    yield 1;
  };
  return inner;
}

async function* lines(stream) { // expect: require-yield
  for await (const chunk of stream) {
    console.log(chunk);
  }
}

// Compliant.
function* good(items) {
  for (const item of items) {
    yield item;
  }
}
function* empty() {}
function* placeholder() {
  throw new Error('override me');
}
function* done() {
  return;
}
function* delegate() {
  yield* good([]);
}
async function* stream() {
  yield await Promise.resolve(1);
}
function plain() {
  return 1;
}
class Tree {
  *walk() {
    visit(this);
    return this.root;
  }
}
const provider = {
  *entries() {
    record();
    return [];
  },
  provide: async function* (request) {
    count(request);
    return { edits: [] };
  },
};
class Leaf {
  children = function* () {
    log();
  };
}
export { ids, gen, lines, good, empty, placeholder, done, delegate, stream, plain, Tree, provider, Leaf };
