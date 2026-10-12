// Getters that end by calling a same-file function that always throws never yield undefined.

function fail(message) {
  report(message);
  throw new Error(message);
}

const failArrow = (message) => {
  throw new TypeError(message);
};

function failLater(message) {
  fail(`wrapped: ${message}`);
}

function sometimes(message) {
  if (!message) {
    return;
  }
  throw new Error(message);
}

function onlyLogs(message) {
  report(message);
}

let replaced = function (message) {
  throw new Error(message);
};
replaced = () => {};

function report(message) {
  return message;
}

const config = {};
Object.defineProperty(config, 'old', {
  get() {
    fail('config.old was removed');
  },
});
Object.defineProperty(config, 'older', {
  get() {
    failArrow('config.older was removed');
  },
});
Object.defineProperty(config, 'oldest', {
  get() {
    failLater('config.oldest was removed');
  },
});

// ---- still reported ----
class Settings {
  get a() { // expect: getter-return
    sometimes('may return');
  }

  get b() { // expect: getter-return
    onlyLogs('returns normally');
  }

  get c() { // expect: getter-return
    replaced('reassigned');
  }

  get d() { // expect: getter-return
    undeclared('not in this file');
  }
}

export { config, Settings };
