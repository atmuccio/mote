function load(text) {
  try {
    return JSON.parse(text);
  } catch (err) {
    throw new Error('invalid configuration'); // expect: preserve-caught-error
  }
}

function load2(text) {
  try {
    return JSON.parse(text);
  } catch (err) {
    if (text.length === 0) {
      throw new TypeError('empty input'); // expect: preserve-caught-error
    }
    log(err);
    throw new RangeError( // expect: preserve-caught-error
      'bad input',
    );
  }
}

async function fetchAll(urls) {
  try {
    return await Promise.all(urls.map(get));
  } catch ({ message }) {
    throw new AggregateError([], 'all failed'); // expect: preserve-caught-error
  }
}

// Quoting the caught error keeps its message but loses its stack (reported since the
// `quoted_message` parameter exists; it is on by default).
function quoted(text) {
  try {
    return JSON.parse(text);
  } catch (err) {
    if (err instanceof SyntaxError) {
      throw new Error('invalid configuration', { cause: err });
    }
    throw new Error(`could not load: ${err.message}`); // expect: preserve-caught-error
  }
}

function quoted2(text) {
  try {
    return JSON.parse(text);
  } catch (err) {
    throw new Error('wrapped: ' + String(err)); // expect: preserve-caught-error
  }
}

// Compliant: the cause is kept, or the throw is elsewhere.

function ok3(text) {
  try {
    return JSON.parse(text);
  } catch {
    throw new Error('no binding to pass');
  }
}

function ok4(text) {
  try {
    return JSON.parse(text);
  } catch (err) {
    throw err;
  }
}

function causeIsSomethingElse(text) {
  try {
    return JSON.parse(text);
  } catch (err) {
    throw new Error('with options', { cause: previous }); // expect: preserve-caught-error
  }
}

function ok6(text) {
  try {
    return JSON.parse(text);
  } catch (err) {
    // Custom errors may not accept a cause; only built-ins are checked.
    throw new ConfigError('invalid configuration');
  }
}

function ok7(items) {
  try {
    return items.map(parse);
  } catch (err) {
    // A nested function runs later, outside the catch.
    return () => {
      throw new Error('later');
    };
  }
}

function ok8(text) {
  try {
    return JSON.parse(text);
  } catch (err) {
    try {
      fallback();
    } catch (inner) {
      throw new Error('fallback failed', { cause: inner });
    }
    throw new Error('first failure', { cause: err });
  }
}

function ok9(text) {
  const Error = makeErrorClass();
  try {
    return JSON.parse(text);
  } catch (err) {
    throw new Error('shadowed constructor');
  }
}

function ok10(text) {
  try {
    return JSON.parse(text);
  } catch (err) {
    const e = new Error('not thrown directly');
    e.inner = err;
    throw e;
  }
}

function ok11(args, text) {
  try {
    return JSON.parse(text);
  } catch (err) {
    throw new Error(...args);
  }
}

try {
  run();
} catch (e) {
  try {
    recover();
  } catch (e2) {
    throw new Error('recovery failed'); // expect: preserve-caught-error
  }
}
