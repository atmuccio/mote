// mote-test: test-file
const assert = require('assert');

describe('parse', () => {
  it('throws', async () => {
    expect(() => parse('')).toThrow(); // expect: unspecific-throw-assertion
    expect(() => parse('')).toThrowError(); // expect: unspecific-throw-assertion
    await expect(load()).rejects.toThrow(); // expect: unspecific-throw-assertion
    expect(() => parse('')).to.throw(); // expect: unspecific-throw-assertion
    (() => parse('')).should.throw(); // expect: unspecific-throw-assertion
    assert.throws(() => parse('')); // expect: unspecific-throw-assertion
    await assert.rejects(load()); // expect: unspecific-throw-assertion

    // Compliant.
    expect(() => parse('')).toThrow(SyntaxError);
    expect(() => parse('')).toThrow(/empty/);
    expect(() => parse('')).toThrow('empty input');
    expect(() => parse('x')).not.toThrow();
    expect(() => parse('x')).to.not.throw();
    expect(() => parse('')).toThrowErrorMatchingSnapshot();
    await expect(load()).rejects.toThrow(NotFound);
    assert.throws(() => parse(''), SyntaxError);
    assert.throws(() => parse(''), { name: 'SyntaxError' });
    assert.doesNotThrow(() => parse('x'));
    emitter.throw();
    stream.throws();
    thing.rejects();
  });
});
