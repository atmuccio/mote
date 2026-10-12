const visible = count > 0 ? true : false; // expect: no-redundant-boolean
const hidden = count > 0 ? false : true; // expect: no-redundant-boolean
const coerced = value ? true : false; // expect: no-redundant-boolean
const same = value ? true : true;
const mixed = value ? true : 0;
if ((a > b) === true) {} // expect: no-redundant-boolean
if (!ready === false) {} // expect: no-redundant-boolean
if (true !== (x instanceof Foo)) {} // expect: no-redundant-boolean
if (Array.isArray(list) == false) {} // expect: no-redundant-boolean
if (Boolean(x) === true) {} // expect: no-redundant-boolean
if ((a > 1 && !b) === true) {} // expect: no-redundant-boolean
if ((a && !b) === true) {}
if ((a ?? b) === true) {}
if (flag === true) {}
if (options.enabled === false) {}
if (call() === true) {}
if (true === false) {}
const isBig = size > 10;
if (isBig === true) {} // expect: no-redundant-boolean
let changing = size > 10;
changing = 'no';
if (changing === true) {}
function local(Boolean) {
  if (Boolean(x) === true) {}
}
if (!!x === true) {} // expect: no-redundant-boolean
