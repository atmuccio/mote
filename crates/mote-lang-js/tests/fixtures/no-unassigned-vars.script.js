// A classic script: top-level variables are globals that other scripts may assign.
var config;
let state;
console.log(config, state);

function local() {
  var inner; // expect: no-unassigned-vars
  return inner;
}
