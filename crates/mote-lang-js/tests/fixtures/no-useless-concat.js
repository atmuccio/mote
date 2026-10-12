const greeting = 'Hello, ' + 'world'; // expect: no-useless-concat
const tpl = `a` + 'b'; // expect: no-useless-concat
const tplSub = `a${x}` + 'b'; // expect: no-useless-concat
const chain = x + 'a' + 'b'; // expect: no-useless-concat
const long = 'first part of a long sentence ' +
  'continued on the next line';
const withVar = 'a' + x;
const numbers = 1 + 2;
const mixed = 'a' + 1;
const nested = 'a' + ('b' + x); // expect: no-useless-concat
const plusInString = 'a+' /* + */ + 'b'; // expect: no-useless-concat
