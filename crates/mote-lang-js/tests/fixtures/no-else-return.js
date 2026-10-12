function sign(n) {
  if (n < 0) {
    return -1;
  } else { // expect: no-else-return
    return 1;
  }
}
function plain(n) {
  if (n) return 1;
  else return 2; // expect: no-else-return
}
function chain(n) {
  if (n === 1) {
    return 'a';
  } else if (n === 2) {
    return 'b';
  } else { // expect: no-else-return
    return 'c';
  }
}
function chainNotAllReturn(n) {
  if (n === 1) {
    log(n);
  } else if (n === 2) {
    return 'b';
  } else {
    return 'c';
  }
  return 'd';
}
function elseIfOnly(n) {
  if (n === 1) {
    return 'a';
  } else if (n === 2) {
    return 'b';
  }
  return 'c';
}
function nestedReturns(n) {
  if (n) {
    if (n > 1) {
      return 2;
    } else { // expect: no-else-return
      return 1;
    }
  } else { // expect: no-else-return
    log(n);
  }
}
function throwsOnly(n) {
  if (!n) {
    throw new Error('x');
  } else {
    return n;
  }
}
function notLast(n) {
  if (n) {
    return 1;
    // after return
  } else { // expect: no-else-return
    run();
  }
}
function returnInLoop(items) {
  if (items) {
    for (const i of items) {
      return i;
    }
  } else {
    return null;
  }
}
function noBraceParent(n) {
  while (n) if (n > 1) return 1; else n--;
}
const arrow = (n) => {
  if (n) {
    return 1;
  }
  // a comment mentioning else
  else { // expect: no-else-return
    return 2;
  }
};
function breakNotReturn(n) {
  for (;;) {
    if (n) {
      break;
    } else {
      n++;
    }
  }
}
