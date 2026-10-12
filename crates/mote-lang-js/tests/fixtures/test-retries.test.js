// mote-test: test-file
jest.retryTimes(3); // expect: test-retries

describe('sync', function () {
  this.retries(2); // expect: test-retries
  it('syncs', { retry: 2 }, async () => {}); // expect: test-retries
  it('pushes', async () => {}, { retry: 3 }); // expect: test-retries
  it('pulls', function () {}).retries(4); // expect: test-retries

  // Compliant.
  jest.retryTimes(0);
  this.retries(0);
  it('fetches', { timeout: 1000 }, async () => {});
  client.retries(3);
  const http = axios.create({ retries: 3 });
  http.configure({ retries: 3 });
});

class Client {
  connect() {
    this.retries(3);
  }
}
