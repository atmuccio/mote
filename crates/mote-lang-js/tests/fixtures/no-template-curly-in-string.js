const user = { name: 'x' };
function format(v) { return v; }
const name = user.name;
const greeting = 'Hello ${name}!'; // expect: no-template-curly-in-string
const ok = `Hello ${name}!`;
const token = '${name}';
const tokens = '${name}${name}';
const unknown = 'Hello ${nobody}!';
const snippet = 'console.log(${1:value});';
const env = 'Path: ${env:HOME}';
const mixed = 'src="${src}" title="${name}"';
const member = 'Name: ${user.name}'; // expect: no-template-curly-in-string
const call = "Total: ${format(name)}"; // expect: no-template-curly-in-string
const code = 'log(`value ${name}`)';
function f(count) {
  throw new Error('Expected ${count} items'); // expect: no-template-curly-in-string
}
class K {
  describe() {
    return 'I am ${this.label}'; // expect: no-template-curly-in-string
  }
}
const empty = 'nothing ${} here';
const unclosed = 'broken ${name';
const dollar = 'costs $5 and {name}';
const number = 'Item ${0}';
