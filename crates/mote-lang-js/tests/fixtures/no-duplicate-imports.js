import { a } from 'mod';
import b from 'mod'; // expect: no-duplicate-imports
import c, { d } from 'other';
import * as e from 'other';
const f = require('mod');
export { a, b, c, d, e, f };
