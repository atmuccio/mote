export function kind(v: string | number): string {
  if (typeof v === 'string') {
    return 'string';
  } else { // expect: no-else-return
    return 'number';
  }
}
namespace N {
  if (flag) {
    return;
  } else { // expect: no-else-return
    run();
  }
}
class C {
  static {
    if (flag) {
      run();
    } else {
      stop();
    }
  }
  get value(): number {
    if (this.x) { return 1; } else if (this.y) { return 2; } else { return 3; } // expect: no-else-return
  }
}
