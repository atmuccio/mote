// A flat configuration that `mote migrate` reads statically (it is never run).
import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import { defineConfig, globalIgnores } from 'eslint/config';
import prettier from 'eslint-config-prettier/flat';
import react from 'eslint-plugin-react';
import globals from 'globals';
import custom from './eslint/custom.js';

const ERROR = 'error';
const jsFiles = ['**/*.js', '**/*.mjs'];
const tsFiles = ['**/*.ts'];
const sourceFiles = [...jsFiles, ...tsFiles];
const strict = process.env.STRICT === '1';
const shared = [{ rules: { 'no-debugger': 'error' } }];
shared.push({ rules: { 'no-debugger': 'off' } });
let level = 'warn';

export default defineConfig([
  globalIgnores(['dist/', '**/*.min.js', '!keep.min.js']),
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    languageOptions: { globals: { ...globals.browser } },
    rules: {
      ...react.configs.flat.recommended.rules,
      eqeqeq: ERROR,
      'no-var': ['error'],
      'no-console': level,
      complexity: ['warn', { max: 12 }],
    },
  },
  {
    files: sourceFiles,
    rules: { '@typescript-eslint/no-explicit-any': 'off' },
  },
  [{ files: ['test/**'], rules: { 'no-console': 'off' } }],
  { files: [`scripts/**/*.${'js'}`], rules: { 'no-console': 'off' } },
  { files: [...tsFiles, custom.files], rules: { 'no-empty': 'off' } },
  { files: ['**/*.tsx'], ...react.configs.flat.recommended, rules: { 'no-var': 'off' } },
  { ...custom, rules: { 'no-debugger': 'off' } },
  ...(strict ? [{ rules: { 'no-alert': 'error' } }] : []),
  ...shared,
  custom.configs.recommended,
  { files: ['**/*.ts'], extends: [tseslint.configs.strict, 'js/recommended'], rules: { 'no-var': 'error' } },
  prettier,
]);
