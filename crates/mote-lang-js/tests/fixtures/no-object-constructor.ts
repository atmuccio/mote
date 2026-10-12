const a: Record<string, number> = new Object() as Record<string, number>; // expect: no-object-constructor
const b = Object<unknown>; // instantiation expression, not a call
const c = new Object<string>(); // expect: no-object-constructor
