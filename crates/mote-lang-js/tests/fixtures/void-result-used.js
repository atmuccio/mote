const users = [];

function findUser(id) {
  users.find((u) => u.id === id);
}

const logAll = () => {
  users.forEach((u) => console.log(u));
  return;
};

const notify = function (user) {
  console.log(user);
};

function withCallback(done) {
  setTimeout(() => {
    done(1);
    return 2; // the callback's return
  });
}

function positives() {
  const user = findUser(7); // expect: void-result-used
  let later;
  later = logAll(); // expect: void-result-used
  if (findUser(1)) { // expect: void-result-used
    console.log(findUser(2).name); // expect: void-result-used
  }
  send(notify(user)); // expect: void-result-used
  const label = `user: ${findUser(3)}`; // expect: void-result-used
  const ok = !findUser(4); // expect: void-result-used
  const both = findUser(5) || later; // expect: void-result-used
  const list = [withCallback(() => {})]; // expect: void-result-used
  const obj = { value: notify(user) }; // expect: void-result-used
  return [user, later, label, ok, both, list, obj];
}

// ---- compliant ----
function returnsValue(id) {
  if (id) {
    return users[id];
  }
}

function fail(message) {
  throw new Error(message);
}

async function load() {
  await Promise.resolve();
}

function* generate() {
  yield 1;
}

let reassigned = function () {};
reassigned = () => 1;

function negatives(flag) {
  findUser(1);
  const value = returnsValue(1);
  if (!flag) return fail('no flag');
  const promise = load();
  const iterator = generate();
  const again = reassigned();
  void findUser(2);
  expect(findUser(3)).toBeUndefined();
  assert.equal(notify(1), undefined);
  const arrow = () => findUser(4);
  const forwarded = flag ? findUser(5) : notify(6);
  (findUser(7), notify(8));
  flag && findUser(9);
  const rendered = <div>{findUser(11)}</div>;
  return [value, promise, iterator, again, arrow, forwarded, rendered];
}

function wrapper() {
  return findUser(10);
}

function send(x) {
  return x;
}

positives();
negatives(true);
wrapper();
