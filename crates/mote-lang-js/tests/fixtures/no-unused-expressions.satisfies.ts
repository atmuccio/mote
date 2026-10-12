// Type-level tests: the statements exist for the type checker.
interface Shape {
  kind: 'circle' | 'square';
}
declare const circle: { kind: 'circle' };
declare const shapes: Shape[];

circle satisfies Shape;
({ kind: 'square' }) satisfies Shape;
shapes[0] satisfies Shape as unknown;
circle.kind satisfies Shape['kind'];

// Still reported: a plain comparison, even in a file with type tests.
circle.kind === 'circle'; // expect: no-unused-expressions
