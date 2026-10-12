import type { Options } from './options';
import { defaults } from './options';
import fs = require('fs');

defaults = {} as Options; // expect: no-import-assign
fs.readFileSync = null as never;
export {};
