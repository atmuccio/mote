var interface = load(); // expect: no-shadow-restricted-names
function run(package, value) { // expect: no-shadow-restricted-names
  var static = package + value; // expect: no-shadow-restricted-names
  return static;
}
var yield = 1; // expect: no-shadow-restricted-names

// ---- compliant ----
var options = { interface: 1, private: 2 };
var privateKey = options.private;
function load() {
  return options.interface + privateKey;
}
