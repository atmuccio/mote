export function area(width: number, height: number): number {
  return width * height;
}

area(1, 2, 3); // expect: extra-arguments

// Overloads: the implementation signature is not the public one.
function pick(value: string): string;
function pick(value: string, fallback: string): string;
function pick(value: string, fallback?: string): string {
  return value || fallback || '';
}
pick('a', 'b');

// `this` parameters are not arguments.
function onClick(this: HTMLElement, event: Event) {
  return this.id + event.type;
}
onClick.call(document.body, new Event('click'));

// Typed function variables must accept what their type allows.
type Handler = (a: number, b: number) => number;
const typed: Handler = (a) => a;
typed(1, 2);

declare function declared(a: number): void;
declared(1, 2);

const generic = <T,>(value: T) => value;
generic<number>(1, 2); // expect: extra-arguments

// Namespace merged with a function.
function merged(a: number) {
  return a;
}
namespace merged {
  export const version = 1;
}
merged(1, 2);
