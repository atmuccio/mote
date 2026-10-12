function f(level) {
  switch (level) {
    case 'debug':
      enableTracing();
    case 'info': // expect: no-fallthrough
      enableLogging();
      break;
    case 'warn':
      notify();
      // falls through
    case 'error':
      alert();
      break;
    case 'a':
    case 'b':
      shared();
      break;
    case 'c':
      one();
      /* fallthrough */
    case 'd':
      two();
      // fall through
    case 'e':
      three();
      // Fall-through is intended here.
    case 'f':
      if (x) {
        return 1;
      } else {
        return 2;
      }
    case 'g':
      if (x) {
        return 1;
      }
    default: // expect: no-fallthrough
      last();
  }
}

function g(x) {
  switch (x) {
    case 1:
      return 1;
    case 2:
      throw new Error('two');
    case 3: {
      doIt();
      break;
    }
    case 4: {
      doIt();
    }
    case 5: // expect: no-fallthrough
      try {
        return run();
      } catch (e) {
        return null;
      }
    case 6:
      for (;;) {
        if (done()) return;
      }
    case 7:
      while (true) {
        step();
      }
    case 8:
      process.exit(1);
    case 9:
      assertNever(x);
    case 10:
      switch (y) {
        case 1:
          return;
        default:
          return;
      }
    case 11:
      doIt();
  }
}

function h(items) {
  for (const x of items) {
    switch (x) {
      case 1:
        continue;
      case 2:
        use(x);
      case 3: // expect: no-fallthrough
        use(x);
    }
  }
}
