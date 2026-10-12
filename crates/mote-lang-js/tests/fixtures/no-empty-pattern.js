const {} = config; // expect: no-empty-pattern
const [] = list; // expect: no-empty-pattern
const { settings: {} } = config; // expect: no-empty-pattern
const { a: [] } = config; // expect: no-empty-pattern
const { settings = {} } = config;
const { a } = config;
const [, second] = list;
const [first] = list;
const { ...rest } = config;
const [...all] = list;
function f({}) {} // expect: no-empty-pattern
function g([]) {} // expect: no-empty-pattern
function h({} = {}) {} // expect: no-empty-pattern
function k(a = {}) {}
({} = x); // expect: no-empty-pattern
[] = x; // expect: no-empty-pattern
for (const {} of items) {} // expect: no-empty-pattern
const obj = {};
const arr = [];
try {} catch ({}) {} // expect: no-empty-pattern
