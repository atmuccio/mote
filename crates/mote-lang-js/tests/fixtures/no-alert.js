alert('Saved!'); // expect: no-alert
if (confirm('Delete?')) {} // expect: no-alert
const name = prompt('Name?'); // expect: no-alert
window.alert('x'); // expect: no-alert
globalThis.confirm('x'); // expect: no-alert
self.prompt('x'); // expect: no-alert
notifications.alert('x');
dialog.confirm('x');
function local(alert) {
  alert('not the global');
}
const confirmFn = () => {};
confirmFn('x');
const other = { alert() {} };
other.alert('x');
