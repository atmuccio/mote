// mote-test: path src/lib/format.js
import { pad } from './format'; // expect: no-self-import
import { trim } from './format.js'; // expect: no-self-import
import other from '../lib/format'; // expect: no-self-import
export { lower } from './format.ts'; // expect: no-self-import
export * from './format.mjs'; // expect: no-self-import

// Compliant.
import { strings } from './strings';
import styles from './format.css';
import data from './format.json';
import types from './format.d.ts';
import tests from './format.test';
import dir from './format/';
import nested from './format/index';
import pkg from 'format';
import scoped from '@scope/format';

export function upper(s) {
  return pad(trim(s)).toUpperCase() + other + strings + styles + data + types + tests + dir + nested + pkg + scoped;
}
