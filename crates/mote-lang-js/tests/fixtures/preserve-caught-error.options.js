// The options object must carry the caught error; quoting it is not enough.

function constantCause(url) {
  try {
    return fetchImage(url);
  } catch (error) {
    throw new Error('failed to fetch image', { cause: 'FETCH_ERROR' }); // expect: preserve-caught-error
  }
}

function noCauseKey(url) {
  try {
    return fetchImage(url);
  } catch (error) {
    throw new TypeError('bad image', { details: url }); // expect: preserve-caught-error
  }
}

function messageOnly(text) {
  try {
    return JSON.parse(text);
  } catch (e) {
    throw new Error(e.message); // expect: preserve-caught-error
  }
}

function optionalMessage(text) {
  try {
    return JSON.parse(text);
  } catch (e) {
    throw new SyntaxError(`bad JSON: ${e?.message}`); // expect: preserve-caught-error
  }
}

function errorAsMessage(text) {
  try {
    return JSON.parse(text);
  } catch (e) {
    throw new Error(e); // expect: preserve-caught-error
  }
}

function conditionalMessage(text) {
  try {
    return JSON.parse(text);
  } catch (e) {
    throw new Error(e instanceof Error ? e.message : 'unknown'); // expect: preserve-caught-error
  }
}

// ---- compliant ----

function causeKept(text) {
  try {
    return JSON.parse(text);
  } catch (e) {
    throw new Error(`bad JSON: ${e.message}`, { cause: e });
  }
}

function shorthandCause(text) {
  try {
    return JSON.parse(text);
  } catch (cause) {
    throw new Error('bad JSON', { cause });
  }
}

function causeDerived(text) {
  try {
    return JSON.parse(text);
  } catch (e) {
    throw new Error('bad JSON', { cause: normalize(e) });
  }
}

function causeLocal(text) {
  try {
    return JSON.parse(text);
  } catch (e) {
    const wrapped = toError(e);
    throw new Error('bad JSON', { cause: wrapped });
  }
}

function spreadOptions(text, extra) {
  try {
    return JSON.parse(text);
  } catch (e) {
    throw new Error('bad JSON', { ...extra });
  }
}

function computedOptions(text, key) {
  try {
    return JSON.parse(text);
  } catch (e) {
    throw new Error('bad JSON', { [key]: e });
  }
}

function optionsVariable(text, options) {
  try {
    return JSON.parse(text);
  } catch (e) {
    throw new Error('bad JSON', options);
  }
}

function aggregateKeepsErrors(tasks) {
  try {
    return runAll(tasks);
  } catch (e) {
    throw new AggregateError([e], `run failed: ${e.message}`);
  }
}

function stackQuoted(text) {
  try {
    return JSON.parse(text);
  } catch (e) {
    throw new Error(`bad JSON\n${e.stack}`);
  }
}

function destructuredMention(text) {
  try {
    return JSON.parse(text);
  } catch ({ message }) {
    throw new Error(`bad JSON: ${message}`);
  }
}

function destructuredOptions(text) {
  try {
    return JSON.parse(text);
  } catch ({ message }) {
    throw new Error('bad JSON', { cause: message });
  }
}

function nestedFunction(text) {
  try {
    return JSON.parse(text);
  } catch (e) {
    return () => {
      throw new Error(e.message);
    };
  }
}

function customError(text) {
  try {
    return JSON.parse(text);
  } catch (e) {
    throw new ParseError(e.message);
  }
}

function outerCause(dir) {
  try {
    convert(dir);
  } catch (error) {
    try {
      cleanup(dir);
    } catch (cleanupError) {
      throw new Error(`conversion and cleanup failed: ${cleanupError.message}`, { cause: error });
    }
    throw error;
  }
}

function aggregateWithOptions(dir) {
  try {
    convert(dir);
  } catch (error) {
    try {
      cleanup(dir);
    } catch (cleanupError) {
      throw new AggregateError([error, cleanupError], 'both failed', { cause: error });
    }
    throw error;
  }
}

function outerCauseUnrelated(dir) {
  try {
    convert(dir);
  } catch (error) {
    try {
      cleanup(dir);
    } catch (cleanupError) {
      throw new Error('cleanup failed', { cause: dir }); // expect: preserve-caught-error
    }
    throw error;
  }
}
