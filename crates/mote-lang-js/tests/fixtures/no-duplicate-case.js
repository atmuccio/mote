switch (action.type) {
  case 'add':
    add();
    break;
  case 'remove':
    remove();
    break;
  case 'add': // expect: no-duplicate-case
    reset();
    break;
  case `remove`: // expect: no-duplicate-case
    break;
  case "Add":
    break;
  default:
    break;
}
switch (n) {
  case 1:
  case 2:
  case 1.0: // expect: no-duplicate-case
  case -1:
  case -1: // expect: no-duplicate-case
  case NaN:
  case Infinity:
  case NaN:
  case Infinity: // expect: no-duplicate-case
    break;
}
switch (true) {
  case a > 0:
    break;
  case a < 0:
    break;
  case a > 0: // expect: no-duplicate-case
    break;
}
switch (kind) {
  case Kind.A:
  case Kind.B:
  case Kind.A: // expect: no-duplicate-case
    break;
  case Other.A:
    break;
}
switch (x) {
  case read():
    break;
  case read():
    break;
}
switch (y) {
  case a:
    break;
  case b:
    break;
}
