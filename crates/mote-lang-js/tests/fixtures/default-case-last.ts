enum Kind { A, B }
function f(kind: Kind): number {
  switch (kind) {
    default: // expect: default-case-last
    case Kind.A: return 1;
    case Kind.B: return 2;
  }
}
