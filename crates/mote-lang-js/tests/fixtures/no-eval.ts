const f = new Function('return this') as () => unknown; // expect: no-eval
setTimeout(() => {}, 1);
