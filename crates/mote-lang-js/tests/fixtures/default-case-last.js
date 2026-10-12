function f(kind) {
  switch (kind) {
    default: other(); break; // expect: default-case-last
    case 'a': a(); break;
  }
  switch (kind) {
    case 'a': a(); break;
    default: other();
  }
  switch (kind) {
    case 'a':
    default: // expect: default-case-last
    case 'b': b(); break;
  }
  switch (kind) {
    case 'a': a(); break;
  }
  switch (kind) {}
  switch (kind) {
    default:
  }
}
