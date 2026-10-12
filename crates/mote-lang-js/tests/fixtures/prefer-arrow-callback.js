items.forEach(function (item) { total += item.price; }); // expect: prefer-arrow-callback
items.forEach((item) => { total += item.price; });
button.addEventListener('click', function () { this.classList.toggle('a'); });
items.map(function () { return arguments.length; });
items.map(async function (x) { return await x; }); // expect: prefer-arrow-callback
items.map(function* gen() { yield 1; });
items.map(function named(x) { return x; });
setTimeout(function () { run(); }.bind(this), 10); // expect: prefer-arrow-callback
setTimeout(function () { run(this); }.bind(this), 10); // expect: prefer-arrow-callback
setTimeout(function () { run(arguments); }.bind(this), 10);
setTimeout(function () { run(); }.bind(other), 10);
const handler = function () { run(); };
(function () { run(); })();
app.controller('Ctrl', function ($scope) { $scope.x = 1; });
app.service('svc', function () { return 1; });
app.factory('fac', function () { return 1; }); // expect: prefer-arrow-callback
new Promise(function (resolve) { resolve(1); }); // expect: prefer-arrow-callback
describe('suite', function () { // nested function mentions this: skipped
  it('works', function () { this.timeout(5000); });
});
items.map(function (x) { return function () { return this; }; });
items.map(function (x) { const thisValue = x; return thisValue; }); // expect: prefer-arrow-callback
items.map(function (x) { return 'this'; });
items.map(function (x) { return x?.y; }); // expect: prefer-arrow-callback
items.map(function (x) { return new.target; });
