// mote-test: test-file
import { test } from '@playwright/test';

test.skip(
  'logs in', // expect: skipped-test-without-reason
  async ({ page }) => {},
);
test.describe.skip(
  'admin', // expect: skipped-test-without-reason
  () => {},
);
test.skip(({ browserName }) => browserName === 'webkit', 'not supported');

function it(name: string) {
  return { skip: (n: string) => n };
}
it('local').skip('x');
