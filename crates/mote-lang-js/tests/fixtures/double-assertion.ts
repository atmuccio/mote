interface User { name: string }
declare const payload: { id: number };
const a = payload as any as User; // expect: double-assertion
const b = payload as {} as User; // expect: double-assertion
const c = payload as unknown as User;
const d = <User><any>payload; // expect: double-assertion
const e = (<any>payload) as User; // expect: double-assertion
const f = payload as any;
const g = payload as unknown as any;
const h = (payload as any) as const;
const i = payload as User as any;
const j = payload as { id: number } as User;
