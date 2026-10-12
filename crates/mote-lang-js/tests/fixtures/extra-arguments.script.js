// A classic script: another script may replace this global function.
function shared(a) {
  return a;
}
shared(1, 2);

(function () {
  function local(a) {
    return a;
  }
  local(1, 2); // expect: extra-arguments
})();
