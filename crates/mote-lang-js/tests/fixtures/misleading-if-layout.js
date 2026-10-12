function convert(value, admin, user) {
  if (value instanceof Map) {
    use(value);
  } if (Array.isArray(value)) { // expect: misleading-if-layout
    use(value);
  }
  if (admin) {
    use(value);
  } else {
    use(user);
  } if (user == null) { // expect: misleading-if-layout
    use(null);
  }
  if (admin) {
    use(value);
  } else if (user) {
    use(user);
  }if (value) { // expect: misleading-if-layout
    use(value);
  }

  if (user)
    if (admin)
      use(user);
  else // expect: misleading-if-layout
    use(value);

  if (user) if (admin) use(user);
  else use(value); // expect: misleading-if-layout
  return value;
}

switch (kind) {
  case 1:
    if (a) {
      run();
    } if (b) { // expect: misleading-if-layout
      run();
    }
    break;
  default:
}

// ---- compliant ----
function negatives(value, admin, user) {
  if (value == null) {
    use(value);
  } else if (admin) {
    use(user);
  }
  if (value == null) {
    use(value);
  }
  if (admin) {
    use(user);
  }
  if (admin) use(value); if (user == null) use(user);
  while (admin) {
    use(value);
  } if (user != null) {
    use(user);
  }
  try {
    use(value);
  } catch (e) {
    use(e);
  } if (user) {
    use(user);
  }
  if (value != null) {
    use(value);
  } /* independent */ if (admin) {
    use(user);
  }
  if (value) { use(value); } if (admin) { use(user); }
  if (user)
    if (admin)
      use(user);
    else
      use(value);
  if (user) {
    if (admin)
      use(user);
  } else
    use(value);
  if (user)
  if (admin)
    use(user);
  else
    use(value);
  const f = function () {
    if (a) {
      b();
    }
  }; if (c) {
    d();
  }
  {
    if (a) {
      b();
    }
  } if (c) {
    d();
  }
}
if(a){b()}if(c){d()}
