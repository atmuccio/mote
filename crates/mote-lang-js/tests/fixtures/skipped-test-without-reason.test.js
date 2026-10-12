// mote-test: test-file
// The findings are reported at the test title; the titles sit on their own lines here because a
// comment on the line of the call would count as the reason.
describe('importer', () => {
  it.skip(
    'imports a large file', // expect: skipped-test-without-reason
    async () => {},
  );
  xit(
    'parses dates', // expect: skipped-test-without-reason
    () => {},
  );
  test.skip(
    'reads headers', // expect: skipped-test-without-reason
    () => {},
  );
  describe.skip(
    'legacy format', // expect: skipped-test-without-reason
    () => {},
  );
  xdescribe(
    'old api', // expect: skipped-test-without-reason
    () => {
      it.skip('inner case, covered by the suite', () => {});
    },
  );

  it('runs', () => {});
  it.skip(
    'pages', // expect: skipped-test-without-reason
    () => {
      load();
      // a comment after the first statement is not the reason
    },
  );

  // Compliant.
  // Flaky on Windows CI, see the tracker.
  it.skip('imports over the network', async () => {});
  it.skip('uploads', async () => {}); // waiting for the new storage API
  it.skip('downloads', async () => {
    // Disabled until the storage emulator supports ranges.
    await download();
  });
  it.skip('streams', () => {
    // Nothing here yet: the streaming API is not merged.
  });
  it.skip('TODO: rewrite for the new parser', () => {});
  it.skip('flaky on CI', () => {});
  it.skip('broken by #1234', () => {});
  it('decides at run time', function () {
    this.skip();
  });
  test.skip(process.platform === 'win32', 'no symlinks on Windows');
  test.skip();
  it.todo('handles empty files');
  test.skip.each([1, 2])('case %i', (n) => {});
});
