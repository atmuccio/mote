// A classic script (no import/export/require): top-level declarations are globals
// shared with other scripts, so they are not reported.
var globalConfig = {};
function globalHelper() {}
class GlobalWidget {}
function local() {
  var unusedLocal = 1; // expect: no-unused-vars
  function nested() {} // expect: no-unused-vars
}
