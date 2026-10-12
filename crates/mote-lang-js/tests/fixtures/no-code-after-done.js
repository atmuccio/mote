describe('callbacks', () => {
  it('asserts after done', (done) => {
    load((err, data) => {
      done();
      expect(data).toBeDefined(); // expect: no-code-after-done
    });
  });

  it('continues after an error branch', (done) => {
    load((err, data) => {
      if (err) done(err);
      expect(data).toBeDefined(); // expect: no-code-after-done
      done();
    });
  });

  it('calls done twice', function (cb) {
    load(() => {
      cb();
      cb(); // expect: no-code-after-done
    });
  });

  beforeEach((next) => {
    next();
    assert.ok(true); // expect: no-code-after-done
  });

  it('continues after a block that ends with done', (done) => {
    load((err) => {
      if (err) {
        done.fail(err);
      }
      assert(err === null); // expect: no-code-after-done
      done();
    });
  });
});

// ---- compliant ----
describe('callbacks done right', () => {
  it('asserts before done', (done) => {
    load((err, data) => {
      expect(data).toBeDefined();
      done();
    });
  });

  it('returns after an error', (done) => {
    load((err, data) => {
      if (err) return done(err);
      if (!data) {
        done(new Error('no data'));
        return;
      }
      expect(data).toBeDefined();
      done();
    });
  });

  it('uses try/catch', (done) => {
    load((data) => {
      try {
        expect(data).toBeDefined();
        done();
      } catch (e) {
        done(e);
      }
    });
  });

  it('cleans up after done', (done) => {
    load((data) => {
      done();
      cleanup(data);
    });
  });

  it('passes done along', (done) => {
    load(done);
    expect(1).toBe(1);
  });

  it('if and else', (done) => {
    load((err) => {
      if (err) {
        done(err);
      } else {
        done();
      }
    });
  });

  it('not a callback test', () => {
    const done = () => {};
    done();
    expect(1).toBe(1);
  });
});

function load(cb) {
  cb(null, {});
}

function cleanup() {}
