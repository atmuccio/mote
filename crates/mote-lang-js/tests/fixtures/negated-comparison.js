if (!(status === 'done')) {} // expect: negated-comparison
if (!(a == b)) {} // expect: negated-comparison
const differs = !(x !== y); // expect: negated-comparison
while (!(n != 0)) {} // expect: negated-comparison

// Compliant.
if (status !== 'done') {}
if (!(a < b)) {}
if (!(a >= b)) {}
if (!a === b) {}
if (!(a && b)) {}
if (!(key in obj)) {}
if (!(x instanceof Y)) {}
const same = !!(a === b);
