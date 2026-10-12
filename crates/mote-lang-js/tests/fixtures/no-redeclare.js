function format(value) {
  return value.trim();
}
function format(value) { // expect: no-redeclare
  return value.toUpperCase();
}

function mixed() {
  var handler = 1;
  function handler() {} // expect: no-redeclare
  return handler;
}

function mixed2() {
  function run() {}
  var run = null; // expect: no-redeclare
  return run;
}

function thrice() {
  function step() {}
  function step() {} // expect: no-redeclare
  function step() {} // expect: no-redeclare
}

// ---- compliant ----

function vars(items) {
  var total = 0;
  var total = items.length;
  return total;
}

function loops(list) {
  for (var i = 0; i < list.length; i++) {}
  for (var i = 0; i < list.length; i++) {}
}

function branches(flag) {
  if (flag) {
    var result = 1;
  } else {
    var result = 2;
  }
  return result;
}

function noValue() {
  function f() {}
  var f;
  return f;
}

function parameter(a) {
  var a = a || {};
  function a() {}
  return a;
}

function caught() {
  try {
    run();
  } catch (e) {
    var e = 1;
  }
}

function blocks(flag) {
  if (flag) {
    function helper() {}
  } else {
    function helper() {}
  }
}

function separate() {
  function inner() {}
}
function separateToo() {
  function inner() {}
}

function namedExpression() {
  var fn = function fn() {
    var fn = 1;
    return fn;
  };
  return fn;
}

var shared = 1;
let other = 2;
