import defaultThing, { count, helper } from './state.js';
import * as utils from './utils.js';

count = 1; // expect: no-import-assign
count++; // expect: no-import-assign
defaultThing = null; // expect: no-import-assign
utils.helper = () => 1; // expect: no-import-assign
delete utils.helper; // expect: no-import-assign
utils['other'] = 2; // expect: no-import-assign

// Compliant.
helper.cache = {};
defaultThing.enabled = true;
utils.nested.value = 3;
let local = count;
local = 2;
console.log(utils.helper(), local);
function shadow(count) {
  count = 2;
  return count;
}
export { shadow };
