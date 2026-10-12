function read() {
  try {
    return parse();
  } finally {
    return null; // expect: no-unsafe-finally
  }
}

function rethrow() {
  try {
    work();
  } catch (e) {
    log(e);
  } finally {
    throw new Error('cleanup failed'); // expect: no-unsafe-finally
  }
}

function loopInFinally(items) {
  try {
    process(items);
  } finally {
    for (const item of items) {
      if (item.done) break;
      if (item.skip) continue;
      item.close();
    }
    switch (items.length) {
      case 0:
        break;
    }
  }
}

function loopAround(items) {
  for (const item of items) {
    try {
      process(item);
    } finally {
      if (item.last) break; // expect: no-unsafe-finally
      continue; // expect: no-unsafe-finally
    }
  }
}

function labels(items) {
  outer: for (const a of items) {
    try {
      use(a);
    } finally {
      inner: for (const b of a) {
        break inner;
      }
      break outer; // expect: no-unsafe-finally
    }
  }
}

function nestedFunction() {
  try {
    work();
  } finally {
    const cb = () => {
      return 1;
    };
    [1, 2].forEach(function (x) {
      if (x) return;
      throw new Error('x');
    });
    cb();
  }
}

function caughtThrow() {
  try {
    work();
  } finally {
    try {
      throw new Error('inner');
    } catch (e) {
      log(e);
    }
  }
}

function uncaughtInnerCatch() {
  try {
    work();
  } finally {
    try {
      cleanup();
    } catch (e) {
      throw e; // expect: no-unsafe-finally
    }
  }
}

function fine() {
  try {
    return work();
  } catch (e) {
    return null;
  } finally {
    cleanup();
  }
}

class K {
  m() {
    try {
      work();
    } finally {
      class Inner {
        n() { return 1; }
      }
      use(Inner);
    }
  }
}
