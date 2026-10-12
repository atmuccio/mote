function properties(style, w, h, base) {
  style.width = w; // expect: overwritten-before-read
  style.width = h;

  style.top = base.top; // expect: overwritten-before-read
  style.left = 0;
  style.top = h;

  config.server.port = base.port; // expect: overwritten-before-read
  config.server.port = 8080;

  arr[0] = w; // expect: overwritten-before-read
  arr[0] = 'b';

  obj['key'] = [w, h]; // expect: overwritten-before-read
  obj['key'] = { nested: true, list: [1, 2] };

  style.border = w ? `solid ${w}` : 'none'; // expect: overwritten-before-read
  style.border = '1px 0';
}

function maps(headers, json, text) {
  headers.set('Accept', json); // expect: overwritten-before-read
  headers.set('Accept', text);

  this.cache.set(1, 'one'); // expect: overwritten-before-read
  this.cache.set(1, 'uno');
}

function thisInPlainFunction(a, b) {
  this.value = a; // expect: overwritten-before-read
  this.value = b;
}

class Plain {
  reset(start) {
    this.count = start; // expect: overwritten-before-read
    this.count = 1;
  }
}

function lambdaIsFine(o, previous) {
  o.handler = previous; // expect: overwritten-before-read
  o.handler = () => o.handler;
}

// ---- compliant ----

async function readsInBetween(style, w, h) {
  style.width = w;
  style.width = style.width + h;

  style.height = w;
  style.height = compute(h);

  style.depth = w;
  log(style.depth);
  style.depth = h;

  a.b = {};
  a.b.c = 1;
  a.b = null;

  a.x.y = w;
  a.x = other;
  a.x.y = 2;

  counter.n = w;
  counter.n += 1;

  obj.p = w;
  obj.p = obj.q;

  m.set('k', 1);
  m.set('k', m.get('k') + 1);

  m.set('a', 1);
  m.set('b', 1);

  m.set(key, 1);
  m.set(key, 2);

  el.value = w;
  el.value = await next();

  a.v = w;
  b.v = 2;

  items[i] = w;
  items[i] = h;
}

function constantsFirst(target, el, msg) {
  target.style.visibility = 'hidden';
  target.style.visibility = 'visible';

  el.src = '';
  el.src = msg;

  el.hidden = true;
  el.hidden = false;

  state.current = undefined;
  state.current = msg;
}

function differentObjects(x, y) {
  x.a = y;
  y.a = 2;
  x.b = y;
  if (y) {
    x.b = 2;
  }
}

function optional(obj, v) {
  obj.a = v;
  obj?.run();
  obj.a = 2;
}

class WithSetter {
  set value(v) {
    this.changed = true;
    this._value = v;
  }

  reset(v) {
    this.value = v;
    this.value = 0;
  }
}

class WithAccessor {
  accessor state = 0;

  update(v) {
    this.state = v;
    this.state = 2;
  }
}

function tooFar(o, v) {
  o.a = v;
  o.b = 1;
  o.c = 1;
  o.d = 1;
  o.e = 1;
  o.a = 2;
}

function newTarget(o, v) {
  o.item = v;
  o.item = new Item();
}

function explained(ctx, color) {
  // Seed a default: the setter silently ignores invalid colors.
  ctx.fillStyle = color.fallback;
  ctx.fillStyle = color.value || 'white';

  ctx.strokeStyle = color.fallback;
  // The setter ignores invalid values, so keep the fallback first.
  ctx.strokeStyle = color.value || 'black';
}
