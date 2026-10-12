items.forEach(function (this: Window, item: string) { use(item); });
items.forEach(function <T>(item: T) { use(item); }); // expect: prefer-arrow-callback
class A extends B {
  m() { items.map(function () { return super.x; }); }
}
