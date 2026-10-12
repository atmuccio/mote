// A classic script (sloppy mode): a parameter may be named `arguments`.
function param(arguments) {
  return arguments[0];
}

function real() {
  return arguments[0]; // expect: prefer-rest-params
}

var top = typeof arguments;
