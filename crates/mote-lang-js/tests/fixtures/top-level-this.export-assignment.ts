// `export =` only exists for CommonJS output, where the top-level `this` is `module.exports`.
const api = { version: 1 };
this.cached = api;
export = api;
