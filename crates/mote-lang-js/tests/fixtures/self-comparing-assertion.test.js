// mote-test: test-file
const assert = require('assert');

describe('cart', () => {
  it('totals', () => {
    const cart = makeCart();
    expect(cart.total).toBe(cart.total); // expect: self-comparing-assertion
    expect(cart.items).toEqual(cart.items); // expect: self-comparing-assertion
    expect(cart.total).not.toBe(cart.total); // expect: self-comparing-assertion
    expect(cart.id).to.equal(cart.id); // expect: self-comparing-assertion
    expect(cart.id).to.deep.equal(cart.id); // expect: self-comparing-assertion
    assert.strictEqual(cart.name, cart.name); // expect: self-comparing-assertion
    assert.deepEqual(cart, cart); // expect: self-comparing-assertion

    // Compliant.
    expect(cart.total).toBe(30);
    expect(cart.total).toBe(other.total);
    expect(make()).toEqual(make());
    expect(1).toBe(1);
    assert.strictEqual(cart.name, 'Ada');
    assert.equal(compute(), compute());
    cart.equal(cart.id, cart.id);
  });
});

describe('container', () => {
  it('returns the same id on subsequent calls', () => {
    expect(container.applicationId).toBe(container.applicationId);
  });
  it('has an id', () => {
    expect(container.applicationId).toBe(container.applicationId); // expect: self-comparing-assertion
  });
});
