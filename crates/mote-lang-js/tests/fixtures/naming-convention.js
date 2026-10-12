'use strict';
// A script (no import or export): its top-level names are globals other scripts call.
const path_utils = require('path');
const { join_paths } = require('./paths');
var global_counter = 0;
function show_popup(msg_text) { // expect: naming-convention
  var local_count = 1; // expect: naming-convention
  return msg_text + local_count;
}
function Legacy(name) {
  this.name = name;
}
Legacy.prototype.do_thing = function do_thing() {};
exports.helper_fn = function helper_fn() {};
module.exports = {
  some_key: function some_key() {},
};
const self_ref = function () {};
(function () {
  const inner_ref = function () {}; // expect: naming-convention
  const __filename2 = 1;
  const ___ = 3;
  const π = 3.14;
  const Größe = {};
  const größe_wert = 1; // expect: naming-convention
  const ok$_ = 2;
  return [inner_ref, __filename2, ___, π, Größe, größe_wert, ok$_];
})();
