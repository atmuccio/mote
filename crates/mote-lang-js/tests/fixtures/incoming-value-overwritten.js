function resize(image, width) {
  log('resizing');
  width = 800; // expect: incoming-value-overwritten
  return scale(image, width);
}

const handler = (event, options = {}) => {
  options = { passive: true }; // expect: incoming-value-overwritten
  listen(event, options);
};

function load(text) {
  try {
    return JSON.parse(text);
  } catch (err) {
    err = new Error('bad input'); // expect: incoming-value-overwritten
    report(err);
  }
}

function each(items) {
  for (let item of items) {
    item = normalize(items[0]); // expect: incoming-value-overwritten
    use(item);
  }
  for (let key in items) {
    if (ready) {
      key = 'x';
    }
    use(key);
  }
}

class Widget {
  render(props) {
    props = this.props; // expect: incoming-value-overwritten
    return props.title;
  }
}

// ---- compliant ----

function readFirst(width) {
  width = width || 800;
  return width;
}

function conditional(width) {
  if (!width) {
    width = 800;
  }
  return width;
}

function logical(width) {
  width ??= 800;
  return width;
}

function viaArguments(a) {
  a = 1;
  return arguments[0];
}

function readByDefault(a, b = a) {
  a = 2;
  return a + b;
}

function hoisted(a) {
  log(peek());
  a = 1;
  function peek() {
    return a;
  }
  return a;
}

function captured(a) {
  const get = () => a;
  a = 1;
  return get();
}

function underscore(_value) {
  _value = 1;
  return _value;
}

function destructured({ a }) {
  return a;
}

function unused(a) {
  return 1;
}

for (const value of list) {
  use(value);
}

for (var v of list) {
  v = 1;
  use(v);
}

try {
  run();
} catch (e) {
  report(e);
  e = null;
}

function compound(n) {
  n += 1;
  return n;
}

function inExpression(n) {
  const m = (n = 5);
  return m + n;
}

function harness(run) {
  return run('editor', 'snippet', 'controller');
}
harness((editor, codeSnippet, controller) => {
  codeSnippet = 'foo$0';
  controller.insert(editor, codeSnippet);
});
