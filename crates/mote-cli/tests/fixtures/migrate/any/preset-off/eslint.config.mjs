// typescript-eslint's recommended configuration, with `@typescript-eslint/no-explicit-any` turned
// off by the team: the rule stays off. The test files' rules come from a local file, which may
// set anything.
import { defineConfig } from 'eslint/config';
import tseslint from 'typescript-eslint';
import testRules from './eslint/test-rules.js';

export default defineConfig([
  ...tseslint.configs.recommended,
  {
    rules: {
      '@typescript-eslint/no-explicit-any': 'off',
      'no-debugger': 'error',
    },
  },
  {
    files: ['test/**/*.ts'],
    extends: [tseslint.configs.recommended],
    rules: { ...testRules },
  },
]);
