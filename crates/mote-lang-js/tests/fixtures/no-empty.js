if (a) {} // expect: no-empty
if (a) {
  // intentionally empty
}
if (a) { foo(); } else {} // expect: no-empty
while (a) {} // expect: no-empty
for (;;) {} // expect: no-empty
for (const x of xs) {} // expect: no-empty
try {} catch (e) {} // expect: no-empty, no-empty
try { foo(); } catch { /* ignored */ } finally {} // expect: no-empty
try {
  foo();
} catch (e) {
  // best effort
}
switch (a) {} // expect: no-empty
switch (a) { /* nothing yet */ }
{} // expect: no-empty
function empty() {}
const arrow = () => {};
class K { m() {} static {} }
const obj = {};
label: {
  // documented
}
