// A classic script: top-level variables are globals that other scripts may read.
var queue = [];
const pending = [];
pending.push(1);

function local() {
  const buffer = []; // expect: unread-collection
  buffer.push(1);
}
local();
