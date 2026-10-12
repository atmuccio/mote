// mote-test: path build/checks.test.ts
import { parse } from './parse';

function check(name: string, ok: boolean): void {
  console.log(ok ? `ok ${name}` : `FAIL ${name}`);
}
check('parses', parse('x') === 1);
