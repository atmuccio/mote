function f(user, items, a, b) {
  if (user) // expect: curly
    greet(user);
  if (!user) return;
  if (user) greet(user); else leave(); // expect: curly, curly
  if (a) {
    one();
  } else two(); // expect: curly
  if (a) {
    one();
  } else if (b) three(); // expect: curly
  if (a) {
    one();
  } else if (b) {
    three();
  } else {
    four();
  }
  if (a) {
    one();
  } /* else here */ else // expect: curly
    two();
  for (const item of items) process(item); // expect: curly
  for (const key in user) {
    use(key);
  }
  for (let i = 0; i < 3; i++) use(i); // expect: curly
  while (a) a = next(a); // expect: curly
  while (b);  // expect: curly
  do step(); while (a); // expect: curly
  do {
    step();
  } while (a);
  outer: for (const x of items) { // fine
    if (x) continue outer;
  }
  if (a && // expect: curly
      b) return;
  if (a) // expect: curly
    return;
  if (a) {
  }
}
