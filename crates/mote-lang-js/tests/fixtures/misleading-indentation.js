function positives(user, items, log) {
  if (!user)
    log('no user');
    return; // expect: misleading-indentation

  for (const item of items)
    log(item);
    log('done'); // expect: misleading-indentation

  while (items.length)
    items.pop();
    log('emptied'); // expect: misleading-indentation

  if (user.admin) {
    log('admin');
  } else
    log('regular');
    log('always'); // expect: misleading-indentation

  if (user.a)
    if (user.b)
      log('both');
    log('only a?'); // expect: misleading-indentation

  outer:
  for (let i = 0; i < 3; i++)
    log(i);
    log('after loop'); // expect: misleading-indentation

  switch (user.kind) {
    case 1:
      if (user.x)
        log('x');
        log('case body'); // expect: misleading-indentation
      break;
    default:
  }
}

// ---- compliant ----
function negatives(user, items, log) {
  if (!user)
    log('no user');
  log('next');

  if (!user) log('no user');
    log('odd but on the same line as the header');

  if (!user) {
    log('no user');
    return;
  }

  for (const item of items)
    log(item);

  log('done');

  if (user.admin)
    log('admin');
  else
    log('regular');
  log('always');

  if (user.admin)
    log('admin');
  else if (user.guest)
    log('guest');
  log('always');

  if (user.a)
    if (user.b)
      log('both');
  log('outside');

  if (user.a)
  log('flat style');
  log('flat style too');

  if (user.a)
    log('one'); log('two');

  while (items.length) items.pop();
    log('emptied');

  for (;;);
    log('empty body is another problem');
}

positives({}, [], console.log);
negatives({}, [], console.log);
