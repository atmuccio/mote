// mote-test: path src/rules/no-foo.test.ts
import { runRuleTests } from '../testing';

describe('no-foo', () => {
  runRuleTests('no-foo', { valid: [], invalid: [] });
});
