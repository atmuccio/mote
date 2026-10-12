async function fetchUser(id) {
  return { id };
}

const saveUser = async (user) => {
  await store(user);
};

async function load(id) {
  try {
    return fetchUser(id); // expect: unawaited-promise-in-try
  } catch (err) {
    return fallback(id);
  }
}

async function loadRemote(url) {
  try {
    return fetch(url).then((r) => r.json()); // expect: unawaited-promise-in-try
  } catch (err) {
    report(err);
    return null;
  }
}

async function withLock(lock, job) {
  await lock.acquire();
  try {
    return new Promise((resolve) => job(resolve)); // expect: unawaited-promise-in-try
  } finally {
    lock.release();
  }
}

async function all(tasks) {
  try {
    return Promise.all(tasks.map(run)); // expect: unawaited-promise-in-try
  } catch (e) {
    return [];
  }
}

async function fireAndForget(user) {
  try {
    saveUser(user); // expect: unawaited-promise-in-try
  } catch (e) {
    report(e);
  }
}

async function chainOnly(user) {
  try {
    notify(user).then(done); // expect: unawaited-promise-in-try
  } catch (e) {
    report(e);
  }
}

async function amongOthers(user) {
  try {
    validate(user);
    saveUser(user);
    notify(user).then(done);
  } catch (e) {
    report(e);
  }
}

class Repo {
  async find(id) {
    return { id };
  }

  async get(id) {
    try {
      return this.find(id); // expect: unawaited-promise-in-try
    } catch (e) {
      return null;
    }
  }
}

// ---- compliant ----

async function awaited(id) {
  try {
    return await fetchUser(id);
  } catch (err) {
    return fallback(id);
  }
}

async function handled(url) {
  try {
    return fetch(url).catch(() => null);
  } catch (err) {
    return null;
  }
}

async function handledThen(url) {
  try {
    fetch(url).then(ok, fail);
  } catch (err) {
    report(err);
  }
}

async function unknownCall(id) {
  try {
    return compute(id);
  } catch (err) {
    return null;
  }
}

function notAsync(id) {
  try {
    return fetchUser(id);
  } catch (err) {
    return null;
  }
}

async function rethrowOnly(id) {
  try {
    return fetchUser(id);
  } catch (err) {
    throw err;
  }
}

async function finallyStatement(id, lock) {
  try {
    fetchUser(id).then(show).catch(report);
  } finally {
    lock.release();
  }
}

async function nestedFunction(ids) {
  try {
    return ids.map((id) => fetchUser(id));
  } catch (err) {
    return [];
  }
}

async function inCatchBlock(id) {
  try {
    await work(id);
  } catch (err) {
    return fetchUser(id);
  }
}

async function voided(user) {
  try {
    void saveUser(user);
  } catch (e) {
    report(e);
  }
}

async function* generator(id) {
  try {
    return fetchUser(id);
  } catch (e) {
    return null;
  }
}

async function cypress() {
  try {
    cy.get('button').then(click);
  } catch (e) {
    report(e);
  }
}

let reassigned = async () => 1;
reassigned = () => 2;
async function callsReassigned() {
  try {
    return reassigned();
  } catch (e) {
    return 0;
  }
}

const refreshQuietly = async () => {
  try {
    await refresh();
  } catch (e) {
    trace(e);
  }
};

async function backgroundRefresh(res) {
  try {
    res.send(cached());
    refreshQuietly();
    return Promise.resolve();
  } catch (e) {
    return Promise.reject(e);
  }
}

async function innerCatchInsideOuterTry(buffer) {
  try {
    try {
      return decode(buffer);
    } catch (error) {
      return fetchUser(buffer.id); // expect: unawaited-promise-in-try
    }
  } catch (error) {
    return {};
  }
}

// ---- retries through the function itself ----

async function download(url, attempt = 0) {
  await ready();
  try {
    const res = await fetch(url);
    if (res.status >= 500 && attempt < 3) {
      return download(url, attempt + 1);
    }
    return res;
  } catch (err) {
    if (attempt < 3) {
      return download(url, attempt + 1);
    }
    throw err;
  }
}

const pollJob = async function (job, left) {
  await tick();
  try {
    return pollJob(job, left - 1);
  } catch (err) {
    return null;
  }
};

async function downloadAll(urls) {
  try {
    return download(urls[0]); // expect: unawaited-promise-in-try
  } catch (err) {
    return null;
  }
}

async function outerRetry(ids) {
  await ready();
  const each = async (id) => {
    try {
      return outerRetry([id]); // expect: unawaited-promise-in-try
    } catch (err) {
      return null;
    }
  };
  return each(ids[0]);
}

// Awaiting here would hold the lock while the inner call waits to acquire it.
async function lockedRetry(lock, n) {
  await lock.acquire();
  try {
    return lockedRetry(lock, n - 1);
  } finally {
    lock.release();
  }
}

class Poller {
  async poll(n) {
    await this.ready;
    try {
      return this.poll(n - 1);
    } catch (e) {
      return null;
    }
  }

  tick = async () => {
    await this.ready;
    try {
      return this.tick();
    } catch (e) {
      return null;
    }
  };

  async other() {
    try {
      return this.poll(1); // expect: unawaited-promise-in-try
    } catch (e) {
      return null;
    }
  }
}

// ---- callees that catch every failure after setup statements ----

async function checkLink(owner, name) {
  if (!owner) {
    return { owner, ok: false };
  }
  const headers = { accept: 'json' };
  if (token) {
    headers.auth = token;
  }
  try {
    const res = await fetch(`/repos/${owner}/${name}`, { headers });
    return { owner, ok: res.ok };
  } catch (err) {
    return { owner, ok: false, error: err.message };
  }
}

async function checkAwaitingSetup(owner) {
  const session = await login(owner);
  try {
    return await fetch(session.url);
  } catch (err) {
    return null;
  }
}

async function checkThrowingSetup(owner) {
  if (!owner) {
    throw new Error('owner required');
  }
  try {
    return await fetch(owner);
  } catch (err) {
    return null;
  }
}

async function checkPromiseGuard(owner) {
  if (!owner) {
    return fetchUser('anonymous');
  }
  try {
    return await fetch(owner);
  } catch (err) {
    return null;
  }
}

async function checkRethrowing(owner) {
  try {
    return await fetch(owner);
  } catch (err) {
    throw err;
  }
}

async function checkAll(owner, name) {
  try {
    if (name) {
      return checkLink(owner, name);
    }
    if (owner.a) return checkAwaitingSetup(owner); // expect: unawaited-promise-in-try
    if (owner.b) return checkThrowingSetup(owner); // expect: unawaited-promise-in-try
    if (owner.c) return checkPromiseGuard(owner); // expect: unawaited-promise-in-try
    return checkRethrowing(owner); // expect: unawaited-promise-in-try
  } catch (err) {
    return null;
  }
}

// ---- promises that only resolve ----

async function ping(url) {
  try {
    const socket = open(url);
    return new Promise((resolve) => {
      socket.onopen = () => resolve(true);
      socket.onerror = () => resolve(false);
    });
  } catch (err) {
    return false;
  }
}

async function pingLegacy(url) {
  try {
    return new Promise(function (resolve) {
      open(url).onclose = resolve;
    });
  } catch (err) {
    return false;
  }
}

async function pingStrict(url) {
  try {
    return new Promise((resolve, reject) => { // expect: unawaited-promise-in-try
      const socket = open(url);
      socket.onopen = () => resolve(true);
      socket.onerror = reject;
    });
  } catch (err) {
    return false;
  }
}

async function pingRest(url) {
  try {
    return new Promise((...callbacks) => open(url, callbacks)); // expect: unawaited-promise-in-try
  } catch (err) {
    return false;
  }
}

// ---- callees whose catch block lets the failure through ----

async function resolveLogged(name) {
  const started = Date.now();
  log(`resolving ${name}`);
  try {
    return await resolveName(name);
  } catch (err) {
    log(`failed after ${Date.now() - started}`, err);
    throw err;
  }
}

async function fetchOrRethrow(url) {
  try {
    return await fetch(url);
  } catch (err) {
    if (isFatal(err)) {
      throw err;
    }
    return null;
  }
}

async function resolveWithRetries(name) {
  for (let attempt = 1; ; attempt++) {
    try {
      if (name.startsWith('http')) {
        return fetchOrRethrow(name); // expect: unawaited-promise-in-try
      }
      return resolveLogged(name); // expect: unawaited-promise-in-try
    } catch (err) {
      if (attempt >= 5) {
        throw err;
      }
    }
  }
}
