if (value === NaN) {} // expect: use-isnan
if (NaN == value) {} // expect: use-isnan
if (value !== Number.NaN) {} // expect: use-isnan
if (value < NaN) {} // expect: use-isnan
if (Number.isNaN(value)) {}
if (isNaN(value)) {}
const x = value + NaN;
switch (NaN) { // expect: use-isnan
  case 1:
    break;
}
switch (value) {
  case NaN: // expect: use-isnan
    break;
  case Number.NaN: // expect: use-isnan
    break;
  default:
    break;
}
function shadowed(NaN) {
  return value === NaN;
}
