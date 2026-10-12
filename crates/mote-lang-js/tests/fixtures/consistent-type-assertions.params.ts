// mote-test: params angle_bracket=false allow_as_argument=true
interface Options { retries: number }
const el = <HTMLElement>document.body;
use({ retries: 2 } as Options);
new Holder({ retries: 2 } as Options);
const options = { retries: 3 } as Options; // expect: consistent-type-assertions
function use(o: Options) { return o; }
class Holder { constructor(o: Options) {} }
