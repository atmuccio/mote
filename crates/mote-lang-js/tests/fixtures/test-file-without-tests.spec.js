// mote-test: test-file
// mote-test: path src/cart.spec.js
import { Cart } from './cart'; // expect: test-file-without-tests

describe('cart', () => {
  // it('adds an item', () => {});
  beforeEach(() => {
    new Cart();
  });
});
