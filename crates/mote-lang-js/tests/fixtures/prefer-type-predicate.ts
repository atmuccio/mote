export function isDate(v: unknown): boolean { // expect: prefer-type-predicate
  return v instanceof Date;
}
export const isString = (v: unknown): boolean => typeof v === 'string'; // expect: prefer-type-predicate
export class Guards {
  private isFn(v: unknown): boolean { // expect: prefer-type-predicate
    return 'function' === typeof v;
  }
  #isNode(v: unknown): boolean { // expect: prefer-type-predicate
    return v instanceof ns.Node;
  }
  private isText = (v: unknown): boolean => typeof v === 'string'; // expect: prefer-type-predicate
  isPublic(v: unknown): boolean {
    return v instanceof Date;
  }
  override matches(other: unknown): boolean {
    return other instanceof Guards;
  }
}
export const guards = {
  isDate(v: unknown): boolean {
    return v instanceof Date;
  },
};

// Compliant.
export function isDate2(v: unknown): v is Date {
  return v instanceof Date;
}
export function inferred(v: unknown) {
  return v instanceof Date;
}
export function negated(v: unknown): boolean {
  return typeof v !== 'string';
}
export function notInstance(v: unknown): boolean {
  return !(v instanceof Date);
}
export function isObject(v: unknown): boolean {
  return typeof v === 'object';
}
export function both(v: unknown): boolean {
  return typeof v === 'string' && v.length > 0;
}
export function other(v: unknown, w: unknown): boolean {
  return typeof w === 'string' || typeof v === 'number';
}
export function outer(v: unknown): boolean {
  return typeof globalValue === 'string';
}
export function destructured({ v }: { v: unknown }): boolean {
  return typeof v === 'string';
}
export function longer(v: unknown): boolean {
  log(v);
  return typeof v === 'string';
}
export declare function declared(v: unknown): boolean;
export abstract class Base {
  abstract check(v: unknown): boolean;
}
