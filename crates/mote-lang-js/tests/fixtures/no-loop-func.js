function bindButtons(buttons) {
  for (var i = 0; i < buttons.length; i++) {
    buttons[i].addEventListener('click', function () {
      select(i); // expect: no-loop-func
    });
  }
}

function timers(items) {
  for (var k = 0; k < items.length; k++) {
    setTimeout(() => process(items[k]), 10); // expect: no-loop-func
  }
}

function bodyVar(items) {
  for (let n = 0; n < items.length; n++) {
    var item = items[n];
    item.load().then(() => show(item)); // expect: no-loop-func
  }
}

function forIn(map) {
  for (var key in map) {
    map[key].onload = () => done(key); // expect: no-loop-func
  }
}

function forOf(list, tasks) {
  for (var entry of list) {
    tasks.push(() => run(entry)); // expect: no-loop-func
  }
}

function outerCounter(list) {
  let i = 0;
  while (i < list.length) {
    window.setTimeout(() => log(list[i])); // expect: no-loop-func
    i++;
  }
}

function outerCursor(head) {
  let node = head;
  do {
    emitter.on('tick', () => visit(node)); // expect: no-loop-func
    node = node.next;
  } while (node);
}

function nodeTick(list) {
  for (var j = 0; j < list.length; j++) {
    process.nextTick(function () {
      use(list[j]); // expect: no-loop-func
    });
  }
}

function jquery(rows) {
  for (var r = 0; r < rows.length; r++) {
    $(rows[r]).on('click', function () {
      highlight(r); // expect: no-loop-func
    });
  }
}

function nestedLoops(grid) {
  for (var x = 0; x < grid.length; x++) {
    for (let y = 0; y < grid[x].length; y++) {
      requestAnimationFrame(() => paint(x, y)); // expect: no-loop-func
    }
  }
}

// ---- compliant ----

function letHead(buttons) {
  for (let i = 0; i < buttons.length; i++) {
    buttons[i].addEventListener('click', () => select(i));
  }
}

function constBody(items) {
  for (var n = 0; n < items.length; n++) {
    const item = items[n];
    setTimeout(() => show(item));
  }
}

function synchronousCallback(items, out) {
  for (var i = 0; i < items.length; i++) {
    items[i].forEach((v) => out.push(v * i));
    const doubled = items.map(() => i * 2);
    use(doubled);
  }
}

function promiseExecutor(items) {
  for (var i = 0; i < items.length; i++) {
    use(new Promise((resolve) => resolve(i)));
  }
}

async function awaited(items) {
  for (var i = 0; i < items.length; i++) {
    await new Promise((resolve) => setTimeout(() => resolve(i), 10));
    await fetchItem(items[i]).then((r) => r[i]);
  }
}

function returned(items) {
  for (var i = 0; i < items.length; i++) {
    if (items[i].ready) {
      return later(() => items[i]).then(() => i);
    }
  }
}

function breaksAfter(items) {
  for (var i = 0; i < items.length; i++) {
    if (items[i].ok) {
      setTimeout(() => pick(i));
      break;
    }
  }
}

function iife(items) {
  for (var i = 0; i < items.length; i++) {
    (function (j) {
      setTimeout(() => use(j));
    })(i);
  }
}

function capturedButNotLoopVar(items, total) {
  for (var i = 0; i < items.length; i++) {
    setTimeout(() => report(total));
  }
}

function outerNotCounter(items) {
  let latest = null;
  for (const item of items) {
    latest = item;
    setTimeout(() => render(latest));
  }
}

function declaredInCallback(items) {
  for (var i = 0; i < items.length; i++) {
    setTimeout(() => {
      var local = items.length;
      use(local);
    });
  }
}

function methodInLoop(items) {
  for (var i = 0; i < items.length; i++) {
    use({
      run() {
        return 1;
      },
    });
  }
}

function notInLoop(value) {
  var v = value;
  setTimeout(() => use(v));
}

function twoShared(canvases) {
  for (var c = 0; c < canvases.length; c++) {
    var context = canvases[c].getContext('2d');
    var picture = new Image();
    picture.onload = function () {
      context.drawImage(picture, 0, 0); // expect: no-loop-func
    };
  }
}
