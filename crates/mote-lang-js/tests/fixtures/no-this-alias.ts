class Store {
  load() {
    const self = this as Store; // expect: no-this-alias
    const { state } = this;
    return function () { return self.load() && state; };
  }
  walk(): TreeNode {
    let node: TreeNode = this;
    while (node.parent) {
      node = node.parent;
    }
    return node;
  }
}
interface TreeNode { parent?: TreeNode }
class Patcher {
  patch(proto: { focus(): void }) {
    const that = this;
    proto.focus = function (this: HTMLElement) {
      that.patch(proto);
    };
  }
}
class Late extends Base {
  constructor() {
    let self: Late | undefined;
    // The callback runs during super(), before `this` exists.
    super(() => self?.ready());
    self = this;
  }
  ready() {}
}
declare class Base { constructor(cb: () => void); }
