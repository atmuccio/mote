// mote-test: params unknown=true
interface User { name: string }
declare const payload: { id: number };
const c = payload as unknown as User; // expect: double-assertion
const a = payload as any as User; // expect: double-assertion
