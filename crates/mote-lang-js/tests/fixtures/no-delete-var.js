// A classic script (sloppy mode), where `delete name` parses.
var session = load();
delete session; // expect: no-delete-var

function reset(cache) {
  delete cache; // expect: no-delete-var
}

// Compliant.
delete registry[name];
delete window.legacy;
delete someImplicitGlobal;
session = null;
reset({});
