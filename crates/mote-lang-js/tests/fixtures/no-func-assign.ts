export function overload(a: string): void;
export function overload(a: number): void;
export function overload(a: unknown): void {}

function plain(): void {}
plain = (): void => {}; // expect: no-func-assign

declare function ambient(): void;
