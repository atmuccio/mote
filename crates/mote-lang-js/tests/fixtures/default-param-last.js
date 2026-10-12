function connect(port = 8080, host) {} // expect: default-param-last
const make = (a, b = 1, c) => {}; // expect: default-param-last
function destructured({ x } = {}, y) {} // expect: default-param-last
class Api {
  call(options = {}, callback) {} // expect: default-param-last
}

// Compliant.
function ok(host, port = 8080) {}
function allDefaults(a = 1, b = 2) {}
function rest(a = 1, ...others) {}
function reducer(state = initialState, action) {}
const todos = (todosState = [], todoAction) => todosState;
items.reduce((acc = 0, x) => acc + x);
promise.then(function (value = null, extra) {});
