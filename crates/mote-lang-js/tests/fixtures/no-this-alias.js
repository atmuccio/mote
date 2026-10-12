function Legacy() {
  var self = this; // expect: no-this-alias
  button.onclick = function () { self.save(); };
}
function Assigned() {
  let that; that = this; // expect: no-this-alias
  return [1].map(function () { return that; });
}
class View {
  render() {
    const me = this; // expect: no-this-alias
    return items.map(function (item) { return me.draw(item); });
  }
  pointless() {
    const self = this; // expect: no-this-alias
    self.draw();
  }
  destructure() {
    const { items, total } = this;
    const [first] = this;
    this.owner = this;
    this.self = this;
    const bound = this.render.bind(this);
    const value = this.value;
    return items + total + first + bound + value;
  }
  cursor() {
    let node = this;
    while (node.parent) {
      node = node.parent;
    }
    return node;
  }
  classExpression() {
    const that = this;
    return new (class extends Base {
      dispose() { that.children.delete(this); }
    })();
  }
  objectMethods() {
    const that = this;
    return {
      call(command) { return that.request(command); },
      get size() { return that.items.length; },
    };
  }
  *generate() {
    const that = this;
    yield* [1, 2].map((x) => x);
    function* inner() { yield that.value; }
    return inner;
  }
}
class Handlers {
  install(el) {
    const that = this;
    el.onclick = function (event) {
      that.log(event);
      return this.id;
    };
    el.onblur = function () {
      that.log(arguments[0]);
    };
    el.onfocus = function () {
      return [1].map(() => that.log(this));
    };
  }
  plain(el) {
    const that = this; // expect: no-this-alias
    el.onclick = function (event) {
      that.log(event);
      return function inner() { return this; };
    };
  }
}
