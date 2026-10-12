try {
  run();
} catch (err) {
  err = new Error('run failed'); // expect: no-ex-assign
  throw err;
}
try {
  run();
} catch (e) {
  const wrapped = new Error('run failed', { cause: e });
  throw wrapped;
}
try {
  run();
} catch ({ message }) {
  message = 'x'; // expect: no-ex-assign
}
try {
  run();
} catch (e) {
  e.message = 'patched';
  [e] = [1]; // expect: no-ex-assign
  const f = () => {
    e = null; // expect: no-ex-assign
  };
  f();
}
try {
  run();
} catch (e) {
  let e2 = e;
  e2 = null;
  function inner(e) {
    e = 1;
  }
  inner(e2);
}
try {
  run();
} catch {
  recover();
}
