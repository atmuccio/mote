export function delay(ms: number): Promise<void> {
  return new Promise<void>(async (resolve) => { // expect: no-async-promise-executor
    await sleep(ms);
    resolve();
  });
}
declare function sleep(ms: number): Promise<void>;
