function totalPrice(items) {
  let sum = 0;
  for (const i of items) sum += i.price * i.qty;
  return sum;
}
function orderValue(items) { // expect: no-identical-functions
  let sum = 0;
  for (const i of items) sum += i.price * i.qty;
  return sum;
}
// Formatting and comments do not matter.
function formatted(items) // expect: no-identical-functions
{
  let sum = 0;   // running total
  for (const i of items)
    sum += i.price * i.qty;
  /* done */ return sum;
}
function different(items) {
  let sum = 0;
  for (const i of items) sum += i.price;
  return sum;
}
function differentString() {
  const a = 'x  y';
  run(a);
  return a;
}
function differentString2() {
  const a = 'x y';
  run(a);
  return a;
}
function quoteStyle() { // expect: no-identical-functions
  const a = "x  y";
  run(a);
  return a;
}
function short() {
  run();
  stop();
}
function shortCopy() {
  run();
  stop();
}
function oneLineButLong() { run(); stop(); wait(); go(); }
function oneLineButLongCopy() { run(); stop(); wait(); go(); }
const handlers = {
  onOpen() {
    this.state = 'open';
    this.notify();
    this.flush();
  },
  onReopen() { // expect: no-identical-functions
    this.state = 'open';
    this.notify();
    this.flush();
  },
};
class Widget {
  render() {
    const el = document.createElement('div');
    el.className = 'widget';
    return el;
  }
  draw = () => { // expect: no-identical-functions
    const el = document.createElement('div');
    el.className = 'widget';
    return el;
  };
}
items.forEach(function (x) {
  if (x) {
    use(x);
  }
});
other.forEach(function (x) { // expect: no-identical-functions
  if (x) {
    use(x);
  }
});
function outer() {
  function inner() {
    a();
    b();
    c();
  }
  inner();
  function inner2() { // expect: no-identical-functions
    a();
    b();
    c();
  }
  return inner2;
}
function thrice() { // expect: no-identical-functions
  a();
  b();
  c();
}
function stub1() {
  throw new Error(
    'not implemented',
  );
}
for (const code of codes) {
  queue.push(() => {
    for (let x = 0; x < 1; x++) {
      draw(code, x);
    }
  });
}
for (const code of others) {
  queue.push(() => {
    for (let x = 0; x < 1; x++) {
      draw(code, x);
    }
  });
}
function setup(editor) {
  const state = compute();
  editor.onA(() => {
    state.run();
    state.stop();
    state.go();
  });
  editor.onB(() => { // expect: no-identical-functions
    state.run();
    state.stop();
    state.go();
  });
}
