throw 'Invalid state'; // expect: no-throw-literal
throw 42; // expect: no-throw-literal
throw { code: 42 }; // expect: no-throw-literal
throw `Missing ${name}`; // expect: no-throw-literal
throw undefined; // expect: no-throw-literal
throw null; // expect: no-throw-literal
throw 'Error: ' + detail; // expect: no-throw-literal
throw [1, 2]; // expect: no-throw-literal
throw cond ? 'a' : 'b'; // expect: no-throw-literal
throw new Error('Invalid state');
throw error;
throw createError();
throw cond ? new Error('a') : 'b';
throw err || 'fallback';
throw this.error;
throw await promise;
function local(undefined) {
  throw undefined;
}
throw (e = new Error('x'));
throw (e = 'x'); // expect: no-throw-literal
