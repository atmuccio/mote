declare const flag: boolean;
if (flag) {}
if (false as boolean) {} // expect: no-constant-condition
if (flag!) {}
export const n = flag ? 1 : 2;
