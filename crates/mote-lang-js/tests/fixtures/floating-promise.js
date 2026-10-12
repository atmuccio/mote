import { remote } from './remote';

async function save(doc) {
  await remote.put(doc);
}

const load = async (id) => remote.get(id);
let refresh = async function () {
  await load(1);
};

function handler(doc) {
  save(doc); // expect: floating-promise
  load(doc.id); // expect: floating-promise
  refresh(); // expect: floating-promise
  fetchUser().then(render); // expect: floating-promise
  fetchUser().then(render).then(done); // expect: floating-promise
  fetchUser().finally(cleanup); // expect: floating-promise
  fetchUser()?.then(render); // expect: floating-promise
}

async function compliant(doc) {
  await save(doc);
  void save(doc);
  const pending = save(doc);
  await pending;
  fetchUser().then(render).catch(showError);
  fetchUser().then(render, showError);
  fetchUser().catch(showError);
  fetchUser().then(render).finally(cleanup).catch(showError);
  return save(doc);
}

// Returned or used as a value.
const loaded = () => save({});
const p = Promise.all([save({}), load(2)]);
items.forEach((item) => save(item));

// Not async: plain functions and generators.
function sync(doc) {
  return doc;
}
sync({});
async function* stream() {
  yield 1;
}
stream();

// Unknown functions are not checked (no type information).
somethingElse();
remote.put({});

// A reassigned variable may no longer hold the async function.
let swap = async () => 1;
swap = () => 2;
swap();

// Cypress chains are not promises.
cy.get('button').then(($btn) => $btn.click());
cy.visit('/').its('title').then((t) => t);

// A `then` without a callback is not a handler chain.
query.then();

// An async function whose whole body is guarded by try/catch does not reject in practice.
async function safeRefresh() {
  try {
    await load(3);
  } catch (error) {
    console.error(error);
  }
}
safeRefresh();

// A try/finally without catch still rejects.
async function unsafe() {
  try {
    await load(4);
  } finally {
    console.log('done');
  }
}
unsafe(); // expect: floating-promise
