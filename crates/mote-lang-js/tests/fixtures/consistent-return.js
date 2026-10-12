function isEditable(doc) {
  if (!doc) {
    return; // expect: consistent-return
  }
  return !doc.readOnly && doc.owner === user;
}

const matches = async (a, b) => {
  if (!a) return; // expect: consistent-return
  if (a === b) return;
  return a.id === b.id || a.key in b;
};

class Store {
  has(key) {
    if (!this.ready) return; // expect: consistent-return
    return this.map.has(key) === true;
  }
  get empty() {
    if (!this.ready) return;
    return false;
  }
  set value(v) {
    if (!v) return;
    this.v = v;
  }
}

// Compliant.
function findPrice(items, id) {
  if (!items) return;
  return items.find((i) => i.id === id).price;
}

function triState(x) {
  if (x == null) return undefined;
  if (!x.ready) return;
  return x.ok === true;
}

function voidOnly(items) {
  if (!items) return;
  items.clear();
  return;
}

function nested(items) {
  items.forEach((i) => {
    if (!i) return;
    use(i);
  });
  return items.length > 0;
}

$('li').each(function () {
  if (skip(this)) return;
  return false;
});

function* gen() {
  if (a) return;
  yield 1;
  return true;
}

function allBooleans(a) {
  if (a) return true;
  return false;
}

function mixedValue(a) {
  if (!a) return;
  if (a.x) return true;
  return a.y;
}
