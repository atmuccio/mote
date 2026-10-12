async function isAllowed(user) {
  return check(user);
}

const loadAll = async () => [1, 2];

function guard(user, items) {
  if (isAllowed(user)) grant(); // expect: no-misused-promises
  if (!isAllowed(user)) deny(); // expect: no-misused-promises
  const label = isAllowed(user) ? 'yes' : 'no'; // expect: no-misused-promises
  while (isAllowed(user) && items.length) items.pop(); // expect: no-misused-promises
  const merged = [...loadAll()]; // expect: no-misused-promises
  const copy = { ...loadAll() }; // expect: no-misused-promises
  const ok = isAllowed(user) || fallback(); // expect: no-misused-promises
  items.filter(async (item) => await check(item)); // expect: no-misused-promises
  items.some(async function (item) { return check(item); }); // expect: no-misused-promises
  items.sort(async (a, b) => a - b); // expect: no-misused-promises
  items.forEach(async (item) => { // expect: no-misused-promises
    await save(item);
  });
}

// ---- compliant ----

async function awaited(user, items) {
  if (await isAllowed(user)) grant();
  const flags = await Promise.all(items.map(async (item) => check(item)));
  const pending = isAllowed(user);
  const fallbackValue = ready && isAllowed(user);
  items.forEach((item) => save(item));
  items.map(async (item) => save(item));
  for (const item of items) await save(item);
  if (sync(user)) grant();
  isAllowed(user).then(grant);
  const list = [...(await loadAll())];
  return [flags, pending, fallbackValue, list];
}

function sync(user) {
  return !!user;
}

let reassigned = async () => true;
reassigned = () => false;
if (reassigned()) grant();

function nested() {
  async function inner() {
    return 1;
  }
  return inner() ? 1 : 2; // expect: no-misused-promises
}
