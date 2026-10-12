function withElse(list) {
  if (list.length === 0) { // expect: no-redundant-boolean
    return true;
  } else {
    return false;
  }
}

function trailing(a, b) {
  if (a > b) return true; // expect: no-redundant-boolean
  return false;
}

function negated(user) {
  if (user.isAdmin()) { // expect: no-redundant-boolean
    return false;
  }
  return true;
}

function truthy(obj) {
  if (obj.value) { // expect: no-redundant-boolean
    return true;
  }
  return false;
}

const arrow = (x) => {
  if (x instanceof Error) return true; // expect: no-redundant-boolean
  else return false;
};

function inSwitch(kind, value) {
  switch (kind) {
    case 'number':
      if (typeof value === 'number') return true; // expect: no-redundant-boolean
      return false;
    default:
      return false;
  }
}

function afterOppositeGuard(s) {
  if (s == null) return false;
  if (s.length > 3) return true; // expect: no-redundant-boolean
  return false;
}

function elseIfBranch(a, b) {
  if (a) {
    return compute(a);
  } else if (b > 1) { // expect: no-redundant-boolean
    return true;
  } else {
    return false;
  }
}

// Guard chains returning the same literal keep their parallel structure.
function equals(a, b) {
  if (a.x !== b.x) return false;
  if (a.y !== b.y) return false;
  return true;
}

function equalsWithDeclaration(a, other) {
  if (!(other instanceof Point)) return false;
  const b = other;
  if (a.x !== b.x) return false;
  return true;
}

function elseIfGuards(a, b) {
  if (a.x !== b.x) {
    return false;
  } else if (a.y !== b.y) {
    return false;
  }
  if (a.z !== b.z) {
    return false;
  }
  return true;
}

function elseIfChainWithFinalElse(a, b) {
  if (a.x !== b.x) {
    return false;
  } else if (a.y !== b.y) {
    return false;
  } else {
    return true;
  }
}

// Not the shape: other values, extra statements, no return after the if.
function sameLiteral(x) {
  if (x) return true;
  return true;
}

function notLiteral(x) {
  if (x) return true;
  return check(x);
}

function moreStatements(x) {
  if (x) {
    log(x);
    return true;
  }
  return false;
}

function codeBetween(x) {
  if (x) return true;
  log(x);
  return false;
}

function nothingAfter(x) {
  if (x) return true;
}

function onlyElseIf(a, b) {
  if (a) return false;
  else if (b) return true;
  return false;
}

function inLoop(items) {
  for (const item of items) {
    if (item.done) {
      return true;
    }
  }
  return false;
}

function returnsStrings(x) {
  if (x) return 'true';
  return 'false';
}
