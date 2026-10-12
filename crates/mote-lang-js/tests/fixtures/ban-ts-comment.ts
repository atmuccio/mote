// expect-next-line: ban-ts-comment
// @ts-nocheck
import { run } from './run';

// expect-next-line: ban-ts-comment
// @ts-ignore
run(1);

// expect-next-line: ban-ts-comment
// @ts-ignore: the typings are wrong
run(2);

// expect-next-line: ban-ts-comment
// @ts-expect-error
run(3);

// expect-next-line: ban-ts-comment
// @ts-expect-error: x
run(4);

// expect-next-line: ban-ts-comment
/* @ts-ignore */
run(5);

// expect-next-line: ban-ts-comment
/// @ts-ignore
run(6);

// @ts-expect-error: the typings miss the numeric overload
run(7);

// @ts-expect-error -- legacy API, removed in v3
run(8);

// A comment that merely mentions @ts-ignore is fine.
run(9);

/**
 * Explains why we do not use @ts-ignore here.
 */
run(10);

// @ts-check
run(11);

// @ts-nocheck only has an effect at the top of the file.
run(12);

const s = '// @ts-ignore';
