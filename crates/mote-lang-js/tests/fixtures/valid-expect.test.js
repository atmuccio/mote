// mote-test: test-file
describe('cart', () => {
  it('computes totals', () => {
    const result = total([1, 2]);
    expect(result); // expect: valid-expect
    expect(result).toBeGreaterThan; // expect: valid-expect
    expect(result).not; // expect: valid-expect
    expect(result).not.toBe; // expect: valid-expect
    expect(result, 'with a message'); // expect: valid-expect
    expect(assertValid(result));
    expect(result.should.be.eql(3));
    expect((flag = true));

    // Compliant.
    expect(result).toBe(3);
    expect(result).not.toBe(4);
    expect(result).to.be.true;
    expect(result).to.exist;
    expect.assertions(1);
    expect(result).toMatchSnapshot();
  });

  it('awaits', async () => {
    await expect(load()).resolves.toEqual({});
  });
});

function localScope() {
  const expect = (v) => v;
  expect(1);
}
localScope();

const expectOption = () => expect(runResult.options);
expectOption().toBe(true);
