export {};
declare var ambient: string;
declare global {
  var globalThing: number;
}
declare module 'library' {
  var inModule: number;
}
declare namespace Legacy {
  var inNamespace: number;
}
namespace Real {
  var inside = 1; // expect: no-var
}
var typed: number = 1; // expect: no-var
