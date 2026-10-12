function ifChain(kind, item) {
  if (kind === 'a') {
    prepare();
    render(item);
  } else if (kind === 'b') { // expect: no-duplicated-branches
    prepare();
    render(item);
  }

  if (kind === 1) {
    start(item);
    finish(item);
  } else if (kind === 2) {
    other(item);
  } else { // expect: no-duplicated-branches
    start(item);
    finish(item);
  }

  if (kind) {
    save(item, {
      force: true,
    });
  } else if (item) { // expect: no-duplicated-branches
    save(item, {
      force: true,
    });
  }

  // Formatting and comments do not matter.
  if (a) {
    x = 1; y = 2;
  } else if (b) { // expect: no-duplicated-branches
    // same thing
    x = 1;
    y = 2;
  }
}

function switchCases(kind) {
  switch (kind) {
    case 'a':
      prepare();
      render();
      break;
    case 'b': // expect: no-duplicated-branches
      prepare();
      render();
      break;
    case 'c':
      return compute(1, 2);
    case 'd':
      other();
      break;
    default: // expect: no-duplicated-branches
      prepare();
      render();
  }
  switch (kind) {
    case 1:
      if (x) {
        y();
      }
      return 1;
    case 2: // expect: no-duplicated-branches
      if (x) {
        y();
      }
      return 1;
  }
}

// ---- compliant ----

function trivial(kind) {
  if (kind === 1) {
    go();
  } else if (kind === 2) {
    go();
  } else {
    go();
  }
  switch (kind) {
    case 1:
      go();
      break;
    case 2:
      go();
      break;
    case 3:
      return true;
    case 4:
      return true;
  }
}

function different(kind) {
  if (kind === 1) {
    prepare();
    render(1);
  } else if (kind === 2) {
    prepare();
    render(2);
  }
  if (a) {
    x = 1;
    y = 2;
  } else if (b) {
    x = 1;
    y = '2';
  }
}

function fallthrough(kind) {
  switch (kind) {
    case 1:
      prepare();
      render();
    case 2:
      prepare();
      render();
      break;
    case 3:
    case 4: // expect: no-duplicated-branches
      prepare();
      render();
      break;
  }
}

function breakDiffers(kind) {
  for (;;) {
    switch (kind) {
      case 1:
        a();
        b();
        break;
      case 2:
        a();
        b();
        continue;
    }
  }
}

function labeledBreak(kind) {
  outer: for (;;) {
    switch (kind) {
      case 1:
        a();
        b();
        break outer;
      case 2:
        a();
        b();
        break;
    }
  }
}

function nested(kind) {
  if (kind) {
    if (x) {
      a();
      b();
    } else {
      c();
      d();
    }
  } else {
    if (x) {
      c();
      d();
    } else {
      a();
      b();
    }
  }
}
