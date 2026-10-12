// mote-test: test-file
import { strict as assert } from 'node:assert';
import { expect } from 'vitest';

it('works', () => {
  expect(42).toBe(answer() as number); // expect: assert-argument-order
  assert.notStrictEqual('x', value); // expect: assert-argument-order
});

function local() {
  const expect = (v: number) => ({ toBe: (w: number) => v === w });
  expect(1).toBe(two);
}
