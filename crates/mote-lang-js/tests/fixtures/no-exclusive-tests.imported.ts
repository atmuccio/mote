// mote-test: test-file
import { describe, it, test } from 'vitest';
import { test as base } from '@playwright/test';

describe.only('imported describe', () => { // expect: no-exclusive-tests
  it('runs', () => {});
});

base.only('playwright', async () => {});
test.only('imported test', () => {}); // expect: no-exclusive-tests

const local = { only(name: string) { return name; } };
local.only('x');
