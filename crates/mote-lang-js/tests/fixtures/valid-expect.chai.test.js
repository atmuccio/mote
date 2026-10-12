import { expect } from 'chai';

describe('chai', () => {
  it('stops early', () => {
    expect(result).to.be; // expect: valid-expect
    expect(list).to.have.length; // expect: valid-expect
    expect(user).to.have.property('name').that.is; // expect: valid-expect
    expect(value).to.equal; // expect: valid-expect
    expect(items).to.deep; // expect: valid-expect
    user.should.have.property; // expect: valid-expect
    user.should.be.an; // expect: valid-expect
    expect(fn).to.throw; // expect: valid-expect
  });

  it('asserts', () => {
    expect(result).to.be.true;
    expect(result).to.exist;
    expect(list).to.have.length(3);
    expect(value).to.equal(1);
    expect(items).to.deep.equal([1]);
    user.should.have.property('name');
    user.should.be.an('object');
    expect(fn).to.throw();
    expect(spy).to.have.been.called;
    expect(promise).to.eventually.be.fulfilled;
    const chain = expect(result).to.be;
    return chain;
  });
});
