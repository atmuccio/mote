function check(user, isOpen, a, b, list) {
  if (user.isAdmin) {
    if (user.isAdmin) grant(); // expect: redundant-nested-condition
  }
  if (isOpen) {
    close();
  } else {
    if (isOpen) retry(); // expect: redundant-nested-condition
  }
  if (a && b) {
    const n = 1;
    if (a) use(n); // expect: redundant-nested-condition
  }
  if (list.length > 0) {
    if (!(list.length > 0)) fail(); // expect: redundant-nested-condition
  }
  if (a) if (a) twice(); // expect: redundant-nested-condition
}

class Gate {
  #ready = false;
  open() {
    if (this.#ready) {
      if (this.#ready) go(); // expect: redundant-nested-condition
    }
  }
}

// ---- compliant ----
function ok(user, a, b, list, i) {
  if (user.isAdmin) {
    if (user.isActive) grant();
  }
  if (user.isAdmin) {
    refresh(user);
    if (user.isAdmin) grant();
  }
  if (a) {
    a = next();
    if (a) use();
  }
  if (list.length) {
    list.pop();
    if (list.length) use();
  }
  if (i < 10) {
    i++;
    if (i < 10) use();
  }
  if (read()) {
    if (read()) use();
  }
  if (a || b) {
    if (a) use();
  }
  if (a) {
    use();
  } else if (a) {
    other();
  }
  if (a) {
    await_(a);
    if (a) use();
  }
  if (a instanceof B) {
    if (a instanceof B) use();
  }
  if (a) {
    const f = () => (a = false);
    if (a) f();
  }
}
