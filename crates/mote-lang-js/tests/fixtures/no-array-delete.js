const items = [];
delete items[0]; // expect: no-array-delete
let config = {};
delete config.x;
var list = [1];
list = {};
delete list[0];
function fn(arr) {
  delete arr[0];
}
