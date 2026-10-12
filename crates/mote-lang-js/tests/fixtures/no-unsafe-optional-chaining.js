// Optional chains whose undefined result is used where it throws.
function unsafe(group, config, order, api, list, Base, key, flag) {
  const all = [...group?.members]; // expect: no-unsafe-optional-chaining
  call(...group?.members); // expect: no-unsafe-optional-chaining
  const { name } = config?.user; // expect: no-unsafe-optional-chaining
  const [head] = list?.items; // expect: no-unsafe-optional-chaining
  for (const item of order?.items) { // expect: no-unsafe-optional-chaining
    use(item);
  }
  (api?.refresh)(); // expect: no-unsafe-optional-chaining
  new (api?.Client)(); // expect: no-unsafe-optional-chaining
  use((config?.user).name); // expect: no-unsafe-optional-chaining
  if (key in config?.map) { // expect: no-unsafe-optional-chaining
    use(key);
  }
  const cond = [...(flag ? group?.a : [])]; // expect: no-unsafe-optional-chaining
  const awaited = async () => (await api?.load()).data; // expect: no-unsafe-optional-chaining
  class Child extends Base?.Parent {} // expect: no-unsafe-optional-chaining
  let x, y;
  [x, y] = list?.pair; // expect: no-unsafe-optional-chaining
  return { all, name, head, cond, awaited, Child, x, y };
}

// Compliant.
function safe(group, config, api, list, boundElement) {
  const all = [...(group?.members ?? [])];
  const { name } = config?.user ?? {};
  const copy = { ...config?.user };
  api?.refresh();
  api?.refresh?.();
  const value = config?.user?.name;
  const len = (list?.items || []).length;
  const mapped = [...(boundElement.boundElements || [])?.filter((e) => e.id)];
  const also = [...[1, 2]?.map((n) => n)];
  const plain = config?.user;
  const test = config?.flag ? 1 : 2;
  for (const item of list?.items ?? []) {
    use(item);
  }
  api?.removeAll([...api?.items.keys()]);
  if (config?.list) {
    for (const item of config?.list) use(item);
  } else {
    for (const item of config?.list) use(item); // expect: no-unsafe-optional-chaining
  }
  const guarded = config?.list && [...config?.list];
  const tern = config?.list ? [...config?.list] : [];
  return { all, name, copy, value, len, mapped, also, plain, test, guarded, tern };
}
function guardedByUse(cells, list, out, database, base) {
  for (const cell of cells) {
    if (cell.kind === 1) out.push(...cell?.outputs);
  }
  if (database.ready) out.push(...base?.items); // expect: no-unsafe-optional-chaining
  return list.filter((e) => e?.items).map((e) => [...e?.items]);
}
export { unsafe, safe, guardedByUse };
