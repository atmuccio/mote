function greet(user) {
  if (user) {
    var name = user.name;
  }
  return hello(name); // expect: block-scoped-var
}

function loops(list) {
  for (var i = 0; i < list.length; i++) {
    var item = list[i];
  }
  use(item); // expect: block-scoped-var
  return i; // expect: block-scoped-var
}

function cases(kind) {
  switch (kind) {
    case 1:
      var label = 'one';
      break;
  }
  return label; // expect: block-scoped-var
}

// ---- compliant ----
function inside(user) {
  if (user) {
    var name = user.name;
    hello(name);
  }
}

function functionLevel(list) {
  var total = 0;
  for (var i = 0; i < list.length; i++) {
    total += list[i];
  }
  var i;
  return total + i;
}

function siblings(a) {
  if (a) {
    var x = 1;
    use(x);
  } else {
    var x = 2;
    use(x);
  }
}

function withLet(user) {
  let name;
  if (user) {
    name = user.name;
  }
  return name;
}

function param(a) {
  if (a) {
    var a = 2;
  }
  return a;
}
