// In TSX, `<T extends unknown>` on an arrow disambiguates it from a JSX tag.
export const identity = <T extends unknown>(x: T) => x;
export function named<T extends unknown>(x: T) { return x; } // expect: no-unnecessary-type-constraint
export const View = () => <div />;
