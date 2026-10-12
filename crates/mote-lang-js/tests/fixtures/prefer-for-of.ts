function f(items: number[], obj: { list: string[] }) {
  let total = 0;
  for (let i = 0; i < items.length; i++) { // expect: prefer-for-of
    total += items[i];
  }
  for (let i = 0; i < obj.list.length; ++i) { // expect: prefer-for-of
    use(obj.list[i]);
  }
  for (let j = 0; j < items.length; j += 1) { // expect: prefer-for-of
    use(items[j], items[j]);
  }
  for (let k = 0; k < items.length; k = k + 1) { // expect: prefer-for-of
    use(items[k]);
  }
  for (let i = 0; i < items.length; i++) {
    console.log(i, items[i]);
  }
  for (let i = 0; i < items.length; i++) {
    items[i] = 0;
  }
  for (let i = 0; i < items.length; i++) {
    items[i]++;
  }
  for (let i = 0; i < items.length; i++) {
    delete items[i];
  }
  for (let i = 0; i < items.length; i++) {
    use(other[i]);
  }
  for (let i = 1; i < items.length; i++) {
    use(items[i]);
  }
  for (let i = 0; i <= items.length; i++) {
    use(items[i]);
  }
  for (let i = 0; i < items.length; i += 2) {
    use(items[i]);
  }
  for (let i = 0; i < items.length; i++) {
    use(items[i + 1]);
  }
  for (let i = 0; i < items.length; i++) {
    run();
  }
  for (let i = 0; i < getItems().length; i++) {
    use(getItems()[i]);
  }
  for (let i = 0; i < items.length; i++) {
    items[i]();
  }
  for (let i = 0, n = items.length; i < n; i++) {
    use(items[i]);
  }
  for (var v = 0; v < items.length; v++) {
    use(items[v]);
  }
  use(v);
  for (var w = 0; w < items.length; w++) { // expect: prefer-for-of
    use(items[w]);
  }
  for (let i = 0; i < items.length; i++) {
    [items[i]] = [1];
  }
  for (let i = 0; i < this.items.length; i++) { // expect: prefer-for-of
    use(this.items[i].name);
  }
  return total;
}
