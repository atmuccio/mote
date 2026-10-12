async function save(store, order) {
  try {
    await store.save(order);
  } catch (err) { // expect: no-useless-catch
    throw err;
  }

  try {
    await store.save(order);
  } catch (err) { // expect: no-useless-catch
    throw err;
  } finally {
    store.close();
  }

  try {
    store.flush();
  }
  catch (e) // expect: no-useless-catch
  {
    // Nothing to add here.
    throw e;
  }
}

function compliant(store, order, log) {
  try {
    store.save(order);
  } catch (err) {
    log.error(err);
    throw err;
  }

  try {
    store.save(order);
  } catch (err) {
    throw new Error('saving failed', { cause: err });
  }

  try {
    store.save(order);
  } catch (err) {
    const other = new Error('x');
    throw other;
  }

  try {
    store.save(order);
  } catch ({ message }) {
    throw message;
  }

  try {
    store.save(order);
  } catch {
    throw lastError;
  }

  try {
    store.save(order);
  } catch (err) {
    err = wrap(err);
    throw err;
  }

  try {
    store.save(order);
  } catch (err) {
    throw err.cause;
  }

  try {
    store.save(order);
  } catch (err) {
    return null;
  }

  try {
    store.save(order);
  } catch (err) {
    if (err.code === 'EEXIST') return;
    throw err;
  }

  try {
    store.save(order);
  } catch (err) {
    {
      const err = new Error('shadow');
      throw err;
    }
  }
}
