function check(items, name, list, set, edit, range) {
  if (items.length >= 0) render(items); // expect: constant-length-comparison
  if (name.length < 0) return; // expect: constant-length-comparison
  if (list.length > -1) use(list); // expect: constant-length-comparison
  if (0 <= items.length) use(items); // expect: constant-length-comparison
  if (list.length <= -1) use(list); // expect: constant-length-comparison
  // Compliant.
  if (items.length > 0) render(items);
  if (items.length === 0) return;
  if (items.length >= 1) use(items);
  if (list?.length >= 0) use(list);
  if (set.size >= 0) use(set);
  if (items.length < 1) return;
  if (items.indexOf(name) < 0) return;
  assert(edit.offset >= 0 && edit.length >= 0);
  if (range.length < 0) throw new Error('negative');
  if (this.length >= 0) use(this);
  assert(items.length >= 0, 'items must be array-like');
}
export { check };
