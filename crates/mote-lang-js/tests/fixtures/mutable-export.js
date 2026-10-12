export let state = {}; // expect: mutable-export
let count = 0;
export function inc() { count++; }
export { count }; // expect: mutable-export
