// Getters that end in a test library's always-failing assertion never yield undefined.
import assert from 'node:assert';
import { expect, should } from 'chai';

test('reads nothing it should not', (t) => {
  const request = {
    get body() {
      t.assert.fail('the body must not be read');
    },
    get headers() {
      assert.fail('the headers must not be read');
    },
    get query() {
      expect.fail('the query must not be read');
    },
    get params() {
      should.fail('the params must not be read');
    },
    get cookies() {
      expect.unreachable('the cookies must not be read');
    },
    get raw() {
      chai.assert.fail('the raw request must not be read');
    },
    get session() {
      mustNotRead('session');
    },
  };
  run(request);
});

// A same-file helper whose body is such an assertion never returns either.
function mustNotRead(what) {
  assert.fail(`${what} must not be read`);
}

// ---- still reported ----
test('records but goes on', (t) => {
  const request = {
    get body() { // expect: getter-return
      t.fail('tap and AVA record the failure and return');
    },
    get headers() { // expect: getter-return
      assert.ok(request.ready, 'may pass');
    },
    get query() { // expect: getter-return
      assert?.fail('optional call: assert may be missing');
    },
    get params() { // expect: getter-return
      logger.fail('not an assertion library');
    },
    get cookies() { // expect: getter-return
      assert.failed('not the fail method');
    },
    get raw() { // expect: getter-return
      fail('a global fail of other runners records and returns');
    },
  };
  run(request);
});
