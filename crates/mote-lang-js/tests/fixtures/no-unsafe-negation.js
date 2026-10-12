if (!key in cache) {} // expect: no-unsafe-negation
if (!value instanceof Date) {} // expect: no-unsafe-negation
if (!(key in cache)) {}
if (!(value instanceof Date)) {}
if ((!key) in cache) {}
if (key in cache) {}
if (!key === cache) {}
const a = !obj.x in y; // expect: no-unsafe-negation
const b = !!x in y; // expect: no-unsafe-negation
