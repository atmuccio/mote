eval('run' + step + '()'); // expect: no-eval
const add = new Function('a', 'b', 'return a + b'); // expect: no-eval
const add2 = Function('return 1'); // expect: no-eval
setTimeout('refresh()', 1000); // expect: no-eval
setInterval(`tick(${n})`, 100); // expect: no-eval
window.setTimeout('x()' + y, 1); // expect: no-eval
window.eval('code'); // expect: no-eval
globalThis.eval(code); // expect: no-eval
setTimeout(refresh, 1000);
setTimeout(() => refresh(), 1000);
setTimeout(handler, 0);
setTimeout(codeString, 0);
steps[step]();
obj.eval('x');
function local(eval) {
  eval('fine');
}
class Function2 {}
new Function2('x');
