const msg = 'Hello ' + name + '!'; // expect: prefer-template
const pre = name + ' says hi'; // expect: prefer-template
const tpl = `a` + b; // expect: prefer-template
const multi = 'a: ' + // expect: prefer-template
  a +
  ' b: ' + b;
const nested = 'x' + (a + 'y'); // expect: prefer-template
const sum = a + b;
const lits = 'a' + 'b';
const num = 1 + 2;
const already = `Hello ${name}!`;
const inCall = f('a' + b); // expect: prefer-template
x += 'suffix';
