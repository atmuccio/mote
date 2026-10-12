export class Router {
  static {
    if (ready) {
      init();
    } if (debug) { // expect: misleading-if-layout
      trace();
    }
  }

  route(path: string, method?: string): number {
    // Every branch above returns: the second `if` behaves like `else if`.
    if (path === '/') {
      return 1;
    } if (path.startsWith('/api')) {
      return 2;
    }
    for (const part of path.split('/')) {
      if (!part) {
        continue;
      } else if (part === '..') {
        break;
      } else {
        throw new Error(part);
      } if (part === '.') {
        trace();
      }
      if (part === 'a') {
        trace();
      } else if (part === 'b') {
        continue;
      } if (part === 'c') { // expect: misleading-if-layout
        trace();
      }
      if (part === 'd') {
        continue;
      } else {
        trace();
      } if (part === 'e') { // expect: misleading-if-layout
        trace();
      }
    }
    if (method)
      if (method === 'GET')
        return 3;
    else // expect: misleading-if-layout
      return 4;
    return 0;
  }
}

namespace Checks {
  if (a) {
    b();
  } if (c) { // expect: misleading-if-layout
    d();
  }
}

declare const ready: boolean, debug: boolean, a: boolean, c: boolean;
declare function init(): void;
declare function trace(): void;
declare function b(): void;
declare function d(): void;
