if (user = currentUser) {} // expect: no-cond-assign
if ((user = currentUser)) {}
if (user === currentUser) {}
while (node = node.parent) {} // expect: no-cond-assign
while ((match = re.exec(text))) {}
while ((line = reader.next()) !== null) {}
do {} while (x = next()); // expect: no-cond-assign
do {} while ((x = next()));
for (; x = next(); ) {} // expect: no-cond-assign
for (; (x = next()); ) {}
for (let i = 0; i < 10; i = i + 1) {}
const v = (a = b) ? 1 : 2; // expect: no-cond-assign
const w = ((a = b)) ? 1 : 2;
if (a && (b = c)) {}
if (( a = b )) {}
if (
  a = b // expect: no-cond-assign
) {}
const f = () => (a = b);
if (a += 1) {} // expect: no-cond-assign
