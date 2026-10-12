if (kind === 'a') {
  handleA();
} else if (kind === 'b') {
  handleB();
} else if (kind === 'a') { // expect: no-dupe-else-if
  handleC();
}

if (a) {
} else if (b) {
} else if (c) {
}

if (a || b) {
  one();
} else if (a) { // expect: no-dupe-else-if
  two();
}

if (a) {
  one();
} else if (a && b) { // expect: no-dupe-else-if
  two();
}

if (a && b) {
  one();
} else if (a) {
  two();
}

if (a) {
  one();
} else if (b || a) {
  two();
} else if (b) { // expect: no-dupe-else-if
  three();
}

if (next()) {
  one();
} else if (next()) {
  two();
}

if (state.done) {
  one();
} else if (advance(state)) {
  two();
} else if (state.done) {
  three();
}

if (x > 1) {
  one();
} else {
  if (x > 1) {
    two();
  }
}

if (x.y) {
} else if (x.y) { // expect: no-dupe-else-if
} else if (x.y) { // expect: no-dupe-else-if
}

if (a) {
  if (b) {
  } else if (b) { // expect: no-dupe-else-if
  }
} else if (c) {
}
