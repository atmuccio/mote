// mote-test: test-file

describe.only('checkout', () => { // expect: no-exclusive-tests
  it.only('adds an item', () => {}); // expect: no-exclusive-tests
  it('removes an item', () => {});
  test.only('totals', () => {}); // expect: no-exclusive-tests
  context.only('when empty', () => {}); // expect: no-exclusive-tests
  specify.only('works', () => {}); // expect: no-exclusive-tests
  suite.only('suite', () => {}); // expect: no-exclusive-tests
});

fit('focused with jasmine', () => {}); // expect: no-exclusive-tests
fdescribe('focused suite', () => {}); // expect: no-exclusive-tests

test.only.each([1, 2])('handles %i', (n) => {}); // expect: no-exclusive-tests
describe.only.each([[1], [2]])('table %i', (n) => {}); // expect: no-exclusive-tests
test.concurrent.only('concurrent', async () => {}); // expect: no-exclusive-tests

// Compliant: normal and skipped tests.
describe('cart', () => {
  it('works', () => {});
  it.skip('skipped', () => {});
  test.each([1, 2])('each %i', (n) => {});
  xit('disabled', () => {});
});

// Not the test framework: a local object with an `only` member.
function helpers() {
  const it = { only: () => {} };
  it.only('not a test');
  const fit = (x) => x;
  fit('local function');
}

// A property called `only` on some other object.
const filters = { only: (x) => x };
filters.only('value');
options.only();
describe.onlyWhen('x', () => {});
