// mote-test: test-file
const assert = require('assert');

describe('cart', () => {
  it('counts', () => {
    expect(3).toBe(items.length); // expect: assert-argument-order
    expect('ready').toEqual(job.state); // expect: assert-argument-order
    expect(-1).not.toBe(list.indexOf(x)); // expect: assert-argument-order
    expect(`done`).toStrictEqual(status()); // expect: assert-argument-order
    assert.equal(3, items.length); // expect: assert-argument-order
    assert.strictEqual('ready', job.state, 'message'); // expect: assert-argument-order
    assert.deepEqual(0, total); // expect: assert-argument-order

    // Compliant.
    expect(items.length).toBe(3);
    expect(1).toBe(1);
    expect(true).toBe(flag);
    expect(null).toBe(value);
    expect(3).toBe(expected);
    expect(3).toBe(fixture.expectedCount);
    expect(3).toBeGreaterThan(items.length);
    expect('abc').toMatch(pattern);
    expect(0).toEqual([]);
    assert.equal(items.length, 3);
    assert.ok(3, items.length);
    assert.equal(3, expectedLength);
    other.equal(3, items.length);
  });
});
