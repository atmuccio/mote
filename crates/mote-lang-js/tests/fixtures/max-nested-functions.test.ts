// mote-test: test-file
// mote-test: params max=1
describe('suite', () => {
  describe('inner', () => {
    beforeEach(() => {
      setup(() => {});
    });
    it('works', async () => {
      await run(async () => {
        await step(() => { // expect: max-nested-functions
          done();
        });
      });
    });
  });
});
