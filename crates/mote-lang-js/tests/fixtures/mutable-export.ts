export let currentUser = null; // expect: mutable-export
export var legacyFlag = false; // expect: mutable-export
export let first = 1, second = 2; // expect: mutable-export
export const fixed = 1;
export function setUser(user: unknown) { currentUser = user; }
let counter = 0;
counter++;
let neverChanged = 1;
let assignedLater;
assignedLater = compute();
const constant = 2;
var oldStyle = 1;
oldStyle = 2;
export { counter }; // expect: mutable-export
export { neverChanged, constant };
export { assignedLater as later, oldStyle }; // expect: mutable-export, mutable-export
export { counter as default }; // expect: mutable-export
export { something } from './elsewhere';
export declare let ambient: number;
declare module 'x' {
  export let inModule: number;
}
namespace Space {
  export let inNamespace = 1;
}
type T = number;
export type { T };
function f() {
  let local = 1;
  local = 2;
  return local;
}
