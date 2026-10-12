const unusedDep = require('dep'); // expect: no-unused-vars
function helper() {} // expect: no-unused-vars
module.exports = function main() {};
