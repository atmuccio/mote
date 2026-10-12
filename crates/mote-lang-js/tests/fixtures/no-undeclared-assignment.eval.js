// Direct eval can declare variables by name: the file is skipped.
function dynamic(code) {
  eval(code);
  result = 1;
  return result;
}
