if (typeof value === 'strnig') {} // expect: valid-typeof
if (typeof value == "fucntion") {} // expect: valid-typeof
if ('array' === typeof value) {} // expect: valid-typeof
if (typeof value !== `nul`) {} // expect: valid-typeof
if (typeof value === undefined) {} // expect: valid-typeof
if (typeof value === 'string') {}
if (typeof value === 'undefined') {}
if (typeof value === "object" || typeof value === 'function') {}
if (typeof value === 'bigint' || typeof value === 'symbol') {}
if (typeof value === typeof other) {}
if (typeof value === kind) {}
if (typeof value < 'z') {}
if (value === 'strnig') {}
function local(undefined) {
  return typeof value === undefined;
}
