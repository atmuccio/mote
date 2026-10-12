// mote-test: params allow_null=false
declare const a: string | null;
if (a == null) {} // expect: eqeqeq
if (a === null) {}
if ((a as unknown) != 1) {} // expect: eqeqeq
