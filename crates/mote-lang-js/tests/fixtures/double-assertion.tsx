interface Props { a: number }
export const C = (p: unknown) => <div>{(p as any as Props).a}</div>; // expect: double-assertion
