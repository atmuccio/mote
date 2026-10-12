function positives(queue, run) {
  while (true) { // expect: no-infinite-loop
    const job = queue.shift();
    if (!job) continue;
    run(job);
  }

  for (;;) { // expect: no-infinite-loop
    for (const job of queue) {
      if (!job) break; // leaves only the inner loop
      run(job);
    }
  }

  do { // expect: no-infinite-loop
    switch (queue.shift()) {
      case 1:
        break; // leaves only the switch
      default:
        run();
    }
  } while (1);

  while (!0) { // expect: no-infinite-loop
    queue.forEach((job) => {
      if (!job) return; // returns from the callback only
      throw new Error('in callback'); // the callback's own throw
    });
  }

  outer: while (true) { // expect: no-infinite-loop
    inner: for (const job of queue) {
      if (job) continue outer; // continues the loop itself
      if (!job) break inner;
    }
  }
}

// ---- compliant ----
function negatives(queue, run) {
  while (true) {
    const job = queue.shift();
    if (!job) break;
    run(job);
  }

  for (;;) {
    if (queue.length === 0) return;
    run(queue.shift());
  }

  while (true) {
    if (!queue.length) throw new Error('empty');
    run(queue.shift());
  }

  outer: for (;;) {
    for (const job of queue) {
      if (!job) break outer;
    }
  }

  outer2: for (;;) {
    if (queue.stopped) break;
    while (true) {
      continue outer2; // continues the outer loop: leaves the inner one
    }
  }

  while (queue.length) {
    run(queue.shift());
  }

  while (true) {
    if (queue.done) process.exit(0);
  }

  try {
    for (;;) {
      run(queue.next()); // ends with an exception from next()
    }
  } catch (e) {
    run(e);
  }

  block: {
    while (true) {
      break block;
    }
  }
}

function* ids() {
  let id = 0;
  while (true) {
    yield id++;
  }
}

async function poll(check) {
  while (true) {
    await check();
  }
}

async function consume(stream) {
  for (;;) {
    for await (const chunk of stream) {
      stream.push(chunk);
    }
  }
}

positives([], () => {});
negatives([], () => {});
ids();
poll(() => {});
consume([]);
