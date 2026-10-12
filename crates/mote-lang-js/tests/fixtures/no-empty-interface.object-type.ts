export function save(options: {}): void {} // expect: no-empty-interface
export type Props = {}; // expect: no-empty-interface
export let value: {} | undefined; // expect: no-empty-interface
export const list: {}[] = []; // expect: no-empty-interface

export interface Module {
  parameters: {}; // expect: no-empty-interface
  properties?: {}; // expect: no-empty-interface
}

export const make = (): {} => ({ a: 1 }); // expect: no-empty-interface

export class Holder {
  private state: {} = {}; // expect: no-empty-interface
}

// Compliant: `{}` as a deliberate tool.
export function isDifferent<T extends {}>(a: T, b: T): boolean {
  return a !== b;
}
export type NonNullish<T> = T & {};
export type SidebarProps<P = {}> = P & { open: boolean };
export type Wrapped = Array<{}>;
export type Check<T> = T extends {} ? true : false;
export type Mapped<T> = { [K in keyof T]: {} extends T[K] ? K : never };
export const record: Record<string, never> = {};
export const obj: object = {};
export type WithMembers = { a: number };
export type Documented = {
  // Filled in by plugins.
};
export const plainObject = {};
const cast = plainObject as {};
export { cast };

declare module 'lib' {
  export const config: {};
}
