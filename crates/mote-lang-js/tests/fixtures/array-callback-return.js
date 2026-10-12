const names = users.map((u) => { // expect: array-callback-return
  u.name.trim();
});
const ok = users.map((u) => {
  return u.name.trim();
});
const expr = users.map((u) => u.name);
users.forEach((u) => { log(u); });
const found = users.find(function (u) { // expect: array-callback-return
  if (u.id === id) {
    console.log(u);
  }
});
const good = users.filter((u) => {
  if (u.active) {
    return true;
  }
  return false;
});
const bare = users.some((u) => { // expect: array-callback-return
  if (u.x) return;
  check(u);
});
const thrower = users.map(() => {
  throw new Error('not implemented');
});
const asyncMap = users.map(async (u) => {
  await save(u);
});
const nested = users.map((u) => { // expect: array-callback-return
  const inner = u.items.map((i) => {
    return i.id;
  });
  store(inner);
});
const sorted = list.sort((a, b) => { // expect: array-callback-return
  a - b;
});
const reduced = list.reduce((acc, x) => { // expect: array-callback-return
  acc.push(x);
}, []);
const fromMapped = Array.from(set, (x) => { // expect: array-callback-return
  x.done = true;
});
const fromOk = Array.from(set, (x) => {
  return x.id;
});
const empty = events.map(() => { });
const voided = stream.map(() => {
  log('changed');
});
const nodeStyle = es.map((file, cb) => {
  cb(null, file);
});
const nodeStyle2 = es.map(function (file, callback) {
  callback(null, file);
});
const withSwitch = items.map((x) => {
  switch (x) {
    case 1:
      return 'one';
    default:
      return 'other';
  }
});
const withTry = items.map((x) => {
  try {
    return parse(x);
  } catch {
    return null;
  }
});
const fnDecl = items.filter((x) => { // expect: array-callback-return
  function helper() {
    return true;
  }
  helper();
});
const chained = items?.map((x) => { // expect: array-callback-return
  x.seen = true;
});
const second = items.map(fn, (x) => {
  x.seen = true;
});
const gen = items.map(function* (x) {
  yield x;
});
