export const bad = new RegExp('(abc' as string); // not a literal after the cast: silent
export const bad2 = new RegExp(')'); // expect: invalid-regexp
export const ok = new RegExp('(abc)', 'g');
