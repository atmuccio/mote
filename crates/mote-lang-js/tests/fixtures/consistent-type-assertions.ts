interface Options { retries: number }
declare const event: { target: unknown };
const el = <HTMLInputElement>event.target; // expect: consistent-type-assertions
const el2 = event.target as HTMLInputElement;
const frozen = <const>['a', 'b']; // expect: consistent-type-assertions
const options = { retries: 3 } as Options; // expect: consistent-type-assertions
const options2 = <Options>{ retries: 3 }; // expect: consistent-type-assertions
const options3: Options = { retries: 3 };
const options4 = { retries: 3 } satisfies Options;
const tuple = { a: 1 } as const;
const loose = { a: 1 } as any;
const unknownObj = { a: 1 } as unknown;
const mock = { retries: 1 } as unknown as Options;
const empty = {} as Record<string, number>; // expect: consistent-type-assertions
use({ retries: 2 } as Options); // expect: consistent-type-assertions
const arr = [] as string[];
const nested = { inner: { a: 1 } as Options }; // expect: consistent-type-assertions
function use(o: Options) { return o; }
