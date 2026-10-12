console.log('x'); // expect: no-console
console.error('x'); // expect: no-console
console['warn']('x'); // expect: no-console
console?.info('x'); // expect: no-console
window.console.log('x');
function f(console) { console.log('local'); }
const logger = { log() {} };
logger.log('x');
const c = console;
export { c, f };
