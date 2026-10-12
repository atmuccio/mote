// mote-test: params allow_named_functions=false
items.map(function named(x) { return x; }); // expect: prefer-arrow-callback
items.map(function recurse(x) { return x ? recurse(x - 1) : 0; });
