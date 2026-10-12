export type Width = 'auto' | string; // expect: no-redundant-type-constituents
export type Input = Config | any; // expect: no-redundant-type-constituents
export type Maybe = unknown | undefined; // expect: no-redundant-type-constituents
export type Nope = string | never; // expect: no-redundant-type-constituents
export type Id = string | number | string; // expect: no-redundant-type-constituents
export type Flag = true | boolean; // expect: no-redundant-type-constituents
export type Count = 0 | -1 | number; // expect: no-redundant-type-constituents
export type Big = 1n | bigint; // expect: no-redundant-type-constituents
export type Tpl = `id-${string}` | string; // expect: no-redundant-type-constituents
export type Both = Config & unknown; // expect: no-redundant-type-constituents
export type Wide = Config & any; // expect: no-redundant-type-constituents
export type Narrowed = 'a' & string; // expect: no-redundant-type-constituents
export type Paren = (Config) | (Config); // expect: no-redundant-type-constituents

export function f(a: 'x' | string): void {} // expect: no-redundant-type-constituents

// Compliant.
export type Sizes = 'auto' | 'fill' | number;
export type Hint = 'auto' | (string & {});
export type Brand = string & { __brand: 'Id' };
export type Never = Config & never;
export type Generic<T> = T | undefined;
export type Keys = keyof Config | string;
export type Nested = Array<string | number> | Array<string | number | boolean>;
export type Record1 = Record<string, any> | undefined;
export type Fn = ((a: string) => void) | null;
export type Mixed = 'a' | 1 | true;
