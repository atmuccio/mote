import { type Foo as Foo } from './types'; // expect: no-useless-rename
import type { Bar as Bar } from './types'; // expect: no-useless-rename
import { Baz } from './types';
export type { Baz as Baz }; // expect: no-useless-rename
export { Baz as Qux };
const { typed: typed }: { typed: number } = { typed: 1 }; // expect: no-useless-rename
