// mote-test: path src/external/lib/thing.ts
// mote-test: params skip_dirs=vendor
function external_fn() {} // expect: naming-convention
export {};
