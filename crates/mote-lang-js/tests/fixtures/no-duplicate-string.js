import { a } from 'some-long-module-name';
import { b } from 'some-long-module-name';
import { c } from 'some-long-module-name';
export { d } from 'some-long-module-name';
const e = require('some-long-module-name');
const f = require('some-long-module-name');

function first() {
  log('Connection refused by server'); // expect: no-duplicate-string
  retry('Connection refused by server');
  throw new Error('Connection refused by server');
}

const config = {
  'long-property-key': 1,
  nested: { 'long-property-key': 2 },
  other: { 'long-property-key': 3 },
};
config['long-property-key'] = 4;
const { 'long-property-key': destructured } = config;

const twice = 'appears only twice';
const again = 'appears only twice';
const short1 = 'short';
const short2 = 'short';
const short3 = 'short';

switch (kind) {
  case 'first-kind-value': // expect: no-duplicate-string
    break;
}
if (kind === 'first-kind-value') {}
if (other === 'first-kind-value') {}

const quoted = "double quoted value"; // expect: no-duplicate-string
const single = 'double quoted value';
const tpl = `double quoted value`;
const again2 = 'double quoted value';

class K {
  'method-with-long-name'() {}
  'field-with-long-name' = 1;
}
const k1 = new K()['method-with-long-name'];
const k2 = new K()['method-with-long-name'];
const k3 = new K()['method-with-long-name'];
const el1 = <div className="container main-layout" />;
const el2 = <div className="container main-layout" />;
const el3 = <div className="container main-layout" />;
