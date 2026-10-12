export function kind(value: number): string {
  switch (value) {
    case 1:
      const label: string = 'one'; // expect: no-case-declarations
      return label;
    case 2:
      type Local = { a: number }; // type-only declarations are erased
      interface Shape { b: number }
      const shape: Local & Shape = { a: 1, b: 2 }; // expect: no-case-declarations
      return String(shape.a);
    case 3: {
      const ok = 'three';
      return ok;
    }
    default:
      return 'many';
  }
}
