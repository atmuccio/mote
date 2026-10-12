if (user) {
  if (user.isAdmin) { // expect: collapsible-if
    grant();
  }
}
if (a)
  if (b) run(); // expect: collapsible-if
if (a) {
  if (b) {
    run();
  } else {
    stop();
  }
}
if (a) {
  if (b) {
    run();
  }
} else {
  stop();
}
if (a) {
  if (b) {
    run();
  }
  stop();
}
if (a) {
  // Only admins get here.
  if (b) {
    run();
  }
}
if (x) {
  run();
} else if (a) {
  if (b) { // expect: collapsible-if
    run();
  }
}
if (a) { if (b) { if (c) { run(); } } } // expect: collapsible-if, collapsible-if
