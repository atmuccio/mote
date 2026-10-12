const a = new Promise(async (resolve) => { // expect: no-async-promise-executor
  resolve(await load());
});
const b = new Promise(async function (resolve, reject) { // expect: no-async-promise-executor
  resolve(1);
});
const c = new Promise((resolve, reject) => {
  load().then(resolve, reject);
});
const d = (async () => load())();
const e = new Promise(executor);
function f(Promise) {
  return new Promise(async () => {});
}
const g = new Other(async () => {});
