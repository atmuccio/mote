function reduce(state, action) {
  switch (action.type) {
    case 'add':
      const item = action.item; // expect: no-case-declarations
      return [...state, item];
    case 'rename':
      let name = action.name; // expect: no-case-declarations
      name = name.trim();
      return state.map((s) => ({ ...s, name }));
    case 'helper':
      function helper() { return 1; } // expect: no-case-declarations
      return helper();
    case 'async':
      async function load() { return 2; } // expect: no-case-declarations
      return load();
    case 'klass':
      class Local {} // expect: no-case-declarations
      return new Local();
    default:
      const fallback = state; // expect: no-case-declarations
      return fallback;
  }
}

function ok(action) {
  switch (action.type) {
    case 'add': {
      const item = action.item;
      return item;
    }
    case 'legacy':
      var old = action.old; // var is function-scoped
      return old;
    case 'call':
      run(() => {
        const inner = 1; // nested function: its own scope
        return inner;
      });
      break;
    case 'if':
      if (action.x) {
        let scoped = action.x;
        return scoped;
      }
      break;
    default: {
      let other = 0;
      return other;
    }
  }
  return null;
}

export { reduce, ok };
