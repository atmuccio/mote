var a = 1; // expect: no-var
let b = 2;
const c = 3;
for (var i = 0; i < 3; i++) {} // expect: no-var
for (let j = 0; j < 3; j++) {}
function f() {
  var x, y; // expect: no-var
  return x + y + a + b + c;
}
export var exported = f(); // expect: no-var
const variable = 'var';
// var in a comment
