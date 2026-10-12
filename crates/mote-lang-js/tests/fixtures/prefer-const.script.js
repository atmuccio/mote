// A classic script: top-level bindings are shared with other scripts.
let sharedTop = 1;
function run() {
  let local = sharedTop; // expect: prefer-const
  return local;
}
{
  let blockScoped = 2; // expect: prefer-const
  run(blockScoped);
}
