'use strict';
// An eslintrc configuration in CommonJS that `mote migrate` reads statically.
const path = require('path');
const tsFiles = ['*.ts', '*.tsx'];
let mode = process.env.CI ? 'error' : 'warn';

module.exports = {
  root: true,
  parserOptions: { tsconfigRootDir: __dirname, project: path.join(__dirname, 'tsconfig.json') },
  extends: ['eslint:recommended', 'plugin:@typescript-eslint/recommended'],
  ignorePatterns: ['build/', 'coverage/'],
  rules: {
    'no-console': 'off',
    ...require('./rules/extra.js'),
    'no-var': 'error',
    'prefer-const': mode,
  },
  overrides: [
    { files: tsFiles, rules: { '@typescript-eslint/no-explicit-any': 'off' } },
    { files: [path.join('test', '**')], rules: { 'no-console': 'off' } },
  ],
};
