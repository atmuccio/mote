// mote-test: test-file
interface Options { retries: number; name: string }
const partial = { retries: 1 } as Options;
const el = <HTMLElement>document.body; // expect: consistent-type-assertions
const nothing = { a: 1 } as never;
