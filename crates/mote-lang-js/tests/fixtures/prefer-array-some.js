function check(users, items, list) {
  if (users.find((u) => u.admin)) grant(); // expect: prefer-array-some
  if (!items.find(function (i) { return i.ok; })) fail(); // expect: prefer-array-some
  const ok = Boolean(list.findLast((x) => x > 0)); // expect: prefer-array-some
  while (queue.find((q) => q.pending)) step(); // expect: prefer-array-some
  const label = list.find((x) => x.id === 1) ? 'yes' : 'no'; // expect: prefer-array-some
  if (ready && users?.find((u) => u.id === id)) go(); // expect: prefer-array-some
  return !!list.find((x) => x); // expect: prefer-array-some
}

// ---- compliant ----
function values(users, $el, repo, list) {
  const admin = users.find((u) => u.admin);
  if (admin) grant(admin);
  if ($el.find('.selected')) highlight();
  if (repo.find({ where: { id: 1 } })) load();
  if (users.find((u) => u.admin) !== undefined) grant();
  const fallback = list.find((x) => x.ok) || defaultItem;
  const chosen = ready && list.find((x) => x.ok);
  if (users.some((u) => u.admin)) grant();
  if (list.find(isValid)) use();
  if (list.findIndex((x) => x.ok) >= 0) use();
  return [fallback, chosen, list.find((x) => x.ok) ?? null];
}

// ---- a local only used as a condition ----
function locals(files, reference, list, users) {
  const found = files.find((f) => f.includes(reference)); // expect: prefer-array-some
  if (!found) {
    throw new Error('not referenced');
  }
  let admin = users.findLast((u) => u.admin); // expect: prefer-array-some
  const label = admin ? 'admin' : 'user';
  const hit = list?.find((x) => x.ok); // expect: prefer-array-some
  while (hit && !done()) step();
  const inner = list.find((x) => x.ok); // expect: prefer-array-some
  run(() => {
    if (inner) go();
  });
  return label;
}

function localValues(files, list, users) {
  const found = files.find((f) => f.ok);
  if (!found) throw new Error('none');
  use(found.name);
  const element = list.find((x) => x.ok);
  return element;
  const unused = list.find((x) => x.ok);
  let reassigned = list.find((x) => x.ok);
  if (!reassigned) reassigned = fallback;
  var hoisted = list.find((x) => x.ok);
  if (hoisted) go();
  const shown = users.find((u) => u.admin);
  const view = shown && render(shown);
  const tested = users.find((u) => u.admin);
  expect(tested).toBeDefined();
  const typed = users.find((u) => u.admin);
  if (typeof typed === 'undefined') go();
  const compared = users.find((u) => u.admin);
  if (compared !== undefined) go();
  const byName = users.find(isAdmin);
  if (!byName) go();
  const { first } = list.find((x) => x.ok) || {};
  if (first) go();
  return [unused, view, reassigned];
}

const topLevel = [1, 2].find((x) => x > 1);
if (topLevel) start();
