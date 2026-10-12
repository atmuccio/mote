// mote-test: test-file
import { test } from '@playwright/test';

test.describe.configure({ retries: 2, mode: 'serial' }); // expect: test-retries
test.describe.configure({ mode: 'parallel' });
