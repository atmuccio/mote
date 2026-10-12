declare global {
  interface Array<T> {
    last(): T | undefined;
  }
}

Array.prototype.last = function <T>(this: T[]): T | undefined { // expect: no-extend-native
  return this[this.length - 1];
};

export function shadowed() {
  const Array = { prototype: {} as Record<string, unknown> };
  Array.prototype.last = 1;
}

// A temporary override in a test is reported; putting the saved original back is not.
export function withFrozenConstructor(run: () => void) {
  const original = Object.getOwnPropertyDescriptor(Object.prototype, 'constructor')!;
  Object.defineProperty(Object.prototype, 'constructor', { // expect: no-extend-native
    configurable: true,
    get: () => original.value,
  });
  try {
    run();
  } finally {
    Object.defineProperty(Object.prototype, 'constructor', original);
  }
}

export function withPatchedPush(run: () => void) {
  const savedPush = Array.prototype.push;
  Array.prototype.push = function (this: unknown[], ...items: unknown[]): number { // expect: no-extend-native
    return savedPush.apply(this, items);
  };
  try {
    run();
  } finally {
    Array.prototype.push = savedPush;
  }
}

let savedAll: PropertyDescriptorMap;
let savedTrim: string['trim'];
export function setUp() {
  savedAll = Object.getOwnPropertyDescriptors(Map.prototype);
  savedTrim = String.prototype['trim'];
}
export function tearDown() {
  Object.defineProperties(Map.prototype, savedAll);
  String.prototype['trim'] = savedTrim;
}

// Not a restore: the saved value comes from somewhere else, or from another property.
const patched = function (this: unknown[]) { return this.length; };
Array.prototype.push = patched; // expect: no-extend-native
const otherDescriptor = Object.getOwnPropertyDescriptor(Array.prototype, 'pop')!;
Object.defineProperty(Array.prototype, 'push', otherDescriptor); // expect: no-extend-native
const fromString = Object.getOwnPropertyDescriptor(String.prototype, 'at')!;
Object.defineProperty(Array.prototype, 'at', fromString); // expect: no-extend-native
let swapped = Array.prototype.includes;
swapped = includesPolyfill;
Array.prototype.includes = swapped; // expect: no-extend-native
