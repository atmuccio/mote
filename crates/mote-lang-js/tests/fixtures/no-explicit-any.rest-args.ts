// mote-test: params ignore_rest_args=true
function a(...args: any[]) {}
function b(...args: Array<any>) {}
function c(...args: readonly any[]) {}
type Fn = (...args: any[]) => void;
function d(x: any, ...rest: string[]) {} // expect: no-explicit-any
function e(...args: Map<any, any>[]) {} // expect: no-explicit-any, no-explicit-any
