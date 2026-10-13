// Unused imports in TypeScript: a name next to used ones goes; an `import type` whose names are
// all unused goes; a value import whose names are all unused is reported without a fix
// (whether it is kept for its side effects depends on the compiler settings).
import { Used, Unused } from './a'; // expect: no-unused-vars
import { OnlyUnused } from './b'; // expect: no-unused-vars
import type { TypeOnly } from './c'; // expect: no-unused-vars
// expect-next-line: no-unused-vars
import Def, * as Spaced from './d';
// expect-next-line: no-unused-vars
import { type Kind, inline } from './e';

export const v: Used = Def(inline);
