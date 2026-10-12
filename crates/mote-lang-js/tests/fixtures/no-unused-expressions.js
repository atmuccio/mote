'use strict';
count + 1; // expect: no-unused-expressions
isReady === true; // expect: no-unused-expressions
x; // expect: no-unused-expressions
'not a directive'; // expect: no-unused-expressions
[a, b]; // expect: no-unused-expressions
({ a: 1 }); // expect: no-unused-expressions
!flag; // expect: no-unused-expressions
typeof x; // expect: no-unused-expressions
a ? b : c; // expect: no-unused-expressions
a && b; // expect: no-unused-expressions
a, b; // expect: no-unused-expressions
`template ${x}`; // expect: no-unused-expressions
count += 1;
count++;
start();
ready && start();
ready || start();
value ?? init();
isValid ? accept() : reject();
void promise;
delete obj.key;
new Widget();
tag`template`;
obj.prop;
obj?.prop;
expect(value).to.be.true;
el.offsetHeight;
!function () {}();
(function () {})();
(() => {})();
a(), b();
[...items];
({ ...spread });
({ [key()]: 1 });
`${compute()}`;
x === compute();
(class { static { init(); } });
async function f() {
  await task;
  'use strict'; // expect: no-unused-expressions
}
function* g() {
  yield 1;
}
const arrow = () => a + b;
const arrowBlock = () => {
  a + b; // expect: no-unused-expressions
};
function h() {
  'use strict';
  'another directive';
  return 1;
}
a.b === c.d; // expect: no-unused-expressions
import('./module');
