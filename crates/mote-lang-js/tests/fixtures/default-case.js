switch (kind) { // expect: default-case
  case 'a':
    runA();
    break;
  case 'b':
    runB();
    break;
}
switch (kind) {
  case 'a':
    runA();
    break;
  default:
    throw new Error(`Unknown kind ${kind}`);
}
switch (kind) {
  default:
    runDefault();
    break;
  case 'a':
    runA();
}
switch (kind) {
  case 'a':
    runA();
    break;
  // no default
}
switch (kind) {
  case 'a':
    runA();
    break;
  // No Default: every kind is handled above.
}
switch (kind) {}
switch (kind) { case 1: run(); } // expect: default-case
