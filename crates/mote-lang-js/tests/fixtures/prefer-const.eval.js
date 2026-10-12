// Direct eval can assign any variable by name, so nothing is reported in this file.
function run(code) {
  let value = 1;
  eval(code);
  return value;
}
export { run };
// Nothing to see.
let unreachable = 1; // no expectation
