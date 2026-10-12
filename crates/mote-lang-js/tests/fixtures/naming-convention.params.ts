// mote-test: params variables=camelCase leading_underscore=false methods=camelCase,PascalCase properties=camelCase,UPPER_CASE allow=snake_ok,Weird_Name local_constants=camelCase enum_members=PascalCase pascal_case_values=true
let _hidden = 1; // expect: naming-convention
let Visible = makeThing(); // expect: naming-convention
let snake_ok = 1;
const PascalValue = 'x';
class Weird_Name {}
class Shape {
  GetArea() { return 1; }
  user_field = 1; // expect: naming-convention
  MAX_SIDES = 4;
  get area_value() { return 1; } // expect: naming-convention
}
function inner() {
  const LOCAL_MAX = 3; // expect: naming-convention
  const localMax = 3;
  return LOCAL_MAX + localMax;
}
enum Mode { Fast, slow_mode, SLOW } // expect: naming-convention
export {};
