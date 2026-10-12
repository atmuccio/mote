import { test as base } from '@playwright/test';

// Playwright reads the fixture names from the first parameter, which must be a destructuring
// pattern even when no fixture is used.
export const test = base.extend<{ registry: string[]; lease: number }>({
  registry: async ({}, use) => {
    await use([]);
  },
  lease: async ({}, use, testInfo) => {
    await use(testInfo.workerIndex);
  },
});

test.beforeAll(async ({}, testInfo) => {
  testInfo.setTimeout(1000);
});

test('runs without fixtures', async ({}) => {
  await Promise.resolve();
});

// Still reported: an empty pattern that is not the first parameter, or in a plain function.
export function plain({}) { // expect: no-empty-pattern
  return 1;
}

export function second(a: number, {}: object) { // expect: no-empty-pattern
  return a;
}

const { settings: {} } = { settings: {} }; // expect: no-empty-pattern
