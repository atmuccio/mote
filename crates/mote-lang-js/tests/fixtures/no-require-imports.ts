const fs = require('fs'); // expect: no-require-imports
import path = require('path'); // expect: no-require-imports
export const lazy = () => require('./lazy'); // expect: no-require-imports
const cfg = require(`./config`).default; // expect: no-require-imports

// Compliant: ES imports, a local `require`, type-only import-equals, `require.resolve`.
import * as os from 'os';
import type Types = require('./types');
import Alias = os.constants;
const resolved = require.resolve('x');
define(['require'], function (require: (id: string) => unknown) {
  const local = require('./local');
});
function withLocal() {
  const require = createRequire(import.meta.url);
  return require('./x');
}
declare function define(deps: string[], fn: (...args: any[]) => void): void;
declare function createRequire(url: string): (id: string) => unknown;
const d = await import('./dynamic');
