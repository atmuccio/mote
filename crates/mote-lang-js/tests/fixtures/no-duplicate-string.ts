type Mode = 'light-theme-mode' | 'dark-theme-mode';
type Other = 'light-theme-mode';
interface Settings {
  'light-theme-mode': string;
  mode: 'light-theme-mode';
}
enum Theme {
  'light-theme-mode' = 'light-theme-value', // expect: no-duplicate-string
  Dark = 'light-theme-value',
}
const value: Mode = 'light-theme-mode'; // expect: no-duplicate-string
const other = 'light-theme-mode';
const third = 'light-theme-mode' as Mode;
const fourth = 'light-theme-value';
declare module 'ambient-module-name' {
  const x: 'ambient-string-value';
  const y = 'ambient-string-value';
  const z = 'ambient-string-value';
  const w = 'ambient-string-value';
}
import type { T } from 'ambient-module-name';
let imp: import('ambient-module-name').X;
import eq = require('ambient-module-name');
