if (a) {
  run();
} else {
  if (b) { // expect: no-lonely-if
    stop();
  }
}
if (a) {
  run();
} else {
  if (b) { // expect: no-lonely-if
    stop();
  } else {
    wait();
  }
}
if (a) {
  run();
} else {
  if (b) {
    stop();
  }
  wait();
}
if (a) {
  run();
} else if (b) {
  stop();
}
if (a) {
  if (b) {
    stop();
  }
}
function f() {
  {
    if (a) {
      run();
    }
  }
}
if (a) run(); else if (b) stop();
