function validate(order) { // expect: invariant-return
  if (!order.items.length) {
    showError('empty order');
    return true;
  }
  return true;
}

const statusOf = (job) => { // expect: invariant-return
  switch (job.state) {
    case 'done':
      return 'ok';
    case 'failed':
      notify(job);
      return "ok";
  }
  return `ok`;
};

let zero = function (x) { // expect: invariant-return
  if (x > 1) return 0;
  if (x < -1) return -0;
  return 0.0;
};

function nothing(x) { // expect: invariant-return
  if (x) return null;
  return null;
}

// Compliant.
function nestedDiffers(list) {
  if (list.some((x) => { if (x) return true; return true; })) return false;
  return true;
}

function differs(order) {
  if (!order.items.length) return false;
  return true;
}

function single() {
  return true;
}

function fallsOff(x) {
  if (x) return true;
  if (!x) return true;
}

function withBareReturn(x) {
  if (x) return;
  return true;
}

function explicitUndefined(x) {
  if (x) return undefined;
  return undefined;
}

function nestedAlsoSame(list) { // expect: invariant-return
  list.forEach(() => { return false; });
  if (list.length) return true;
  return true; // the arrow returns false, but for itself
}

function computed(x) {
  if (x) return x;
  return true;
}

function* gen() {
  if (a) return 1;
  return 1;
}

const handlers = {
  onClick() {
    if (a) return false;
    return false;
  },
};

class Visitor {
  visit(node) {
    if (node.skip) return true;
    return true;
  }
}

element.onsubmit = function () {
  if (a) return false;
  return false;
};

$('a').each(function () {
  if (a) return true;
  return true;
});

const [destructured] = [function () { if (a) return 1; return 1; }];

function advance(opts) {
  if (opts.stop) return false;
  opts.next();
  return false;
}
$('a').on('click', function () { return advance(opts); });

function asHandler(e) {
  if (e.x) return false;
  return false;
}
el.addEventListener('click', asHandler);

const write = (chunk) => {
  if (chunk) return true;
  return true;
};
process.stdout.write = write;

function awaited() {
  if (a) return 1;
  return 1;
}
async function relay() { return await awaited(); }

const arrowRelay = () => awaited();

function calledOnly(x) { // expect: invariant-return
  if (x) return 'done';
  return 'done';
}
if (calledOnly(1)) { calledOnly(2); }
