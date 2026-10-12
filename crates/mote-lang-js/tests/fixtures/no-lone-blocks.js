function run() {
  prepare();
  { // expect: no-lone-blocks
    execute();
  }
  {
    const tmp = load();
    use(tmp);
  }
  {
    let x = 1;
    use(x);
  }
  {
    class Local {}
    use(Local);
  }
  {
    function helper() {}
    helper();
  }
  {
  }
  { // expect: no-lone-blocks
    var hoisted = 1;
  }
}
{ // expect: no-lone-blocks
  topLevel();
}
{ { // expect: no-lone-blocks, no-lone-blocks
  nested();
} }
label: {
  run();
  break label;
}
switch (x) {
  case 1: {
    run();
    break;
  }
  case 2:
    run();
    { // expect: no-lone-blocks
      stop();
    }
    break;
}
if (x) {
  run();
}
class K {
  static {
    { // expect: no-lone-blocks
      init();
    }
  }
}
function sections() {
  // Setup
  {
    prepare();
  }
  //#region render
  {
    render();
  }
  //#endregion
  after(); { // expect: no-lone-blocks
    more();
  }
}
