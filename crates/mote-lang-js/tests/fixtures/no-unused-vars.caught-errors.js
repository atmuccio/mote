// mote-test: params caught_errors=true
export function load(read) {
  try {
    return read();
  } catch (err) { // expect: no-unused-vars
    return null;
  }
}

export function used(read, log) {
  try {
    return read();
  } catch (err) {
    log(err);
    return null;
  }
}

export function ignored(read) {
  try {
    return read();
  } catch (_err) {
    return null;
  }
}

export function noBinding(read) {
  try {
    return read();
  } catch {
    return null;
  }
}

export function destructured(read) {
  try {
    return read();
  } catch ({ message }) {
    return null;
  }
}

export function nested(read) {
  try {
    return read();
  } catch (outer) {
    try {
      return read();
    } catch (inner) { // expect: no-unused-vars
      return outer;
    }
  }
}
