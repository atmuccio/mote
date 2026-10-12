// mote-test: params max=3
load(id, (err, doc) => {
  parse(doc, (err, tree) => {
    render(tree, (err, html) => {
      save(html, (err) => { // expect: max-nested-functions
        notify(() => done());
        list.map((x) => x.id);
      });
    });
  });
});

function outer() {
  return function middle() {
    return {
      method() {
        return () => { // expect: max-nested-functions
          return 1;
        };
      },
    };
  };
}

(function () {
  define(['dep'], function (dep) {
    return function a() {
      return function b() {
        return function c() {};
      };
    };
  });
})();

class Widget {
  render() {
    return items.map((item) => {
      return () => item.onClick(); // three levels: fine
    });
  }
}

// UMD: the factory handed to the wrapper does not add a level either.
(function (factory) {
  if (typeof define === 'function' && define.amd) {
    define(['jquery'], factory);
  } else {
    factory(jQuery);
  }
}(function ($) {
  $.fn.plugin = function () {
    this.each(function () {
      $(this).on('click', function () {
        return 1;
      });
    });
  };
  $.fn.deep = function () {
    this.each(function () {
      $(this).on('click', function () {
        setTimeout(function () { // expect: max-nested-functions
          return 2;
        });
      });
    });
  };
}));

(function (root, factory) {
  root.lib = factory();
})(this, function () {
  return function a() {
    return function b() {
      return function c() {};
    };
  };
});

!function (factory) {
  factory();
}(() => {
  run(() => {
    run(() => {
      run(() => {});
    });
  });
});

function host() {
  apply(function (callback) {
    callback();
  }, function () {
    run(() => {
      run(() => { // expect: max-nested-functions
        return 3;
      });
    });
  });
}
