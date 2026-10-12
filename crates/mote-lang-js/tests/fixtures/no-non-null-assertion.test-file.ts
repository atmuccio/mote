// mote-test: test-file
// In tests, an assertion failing is the point: not reported.
declare const result: { value?: number } | undefined;
expect(result!.value!).toBe(1);
declare function expect(v: unknown): { toBe(x: unknown): void };
