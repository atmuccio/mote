// No import/export: TypeScript treats this file as a global script.
const sharedConstant = 1;
function sharedHelper(): void {}
interface SharedShape { a: number }
function local(): void {
  const unusedLocal = 1; // expect: no-unused-vars
}
