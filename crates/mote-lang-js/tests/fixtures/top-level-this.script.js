// A script: the top-level `this` depends on how the file is loaded.
this.legacy = true;
(function (root) {
  root.lib = {};
})(this);
