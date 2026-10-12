// mote-test: test-file
import { strict as assert } from 'node:assert';
import { expect as chaiExpect } from 'chai';

it('throws', () => {
  expect(() => run()).toThrow(); // expect: unspecific-throw-assertion
  assert.throws(() => run()); // expect: unspecific-throw-assertion
  expect(() => run()).toThrow(Error);
});

function local() {
  const expect = (x: unknown) => ({ toThrow: () => x });
  expect(1).toThrow();
}
