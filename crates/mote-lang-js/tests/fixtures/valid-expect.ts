import { expect, test } from 'vitest';

test('typed', () => {
  const value: number = 1;
  expect(value as number); // expect: valid-expect
  expect(value).toEqual; // expect: valid-expect
  expect(value).toEqual(1);
});
