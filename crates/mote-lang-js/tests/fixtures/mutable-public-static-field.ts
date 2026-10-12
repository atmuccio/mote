type Handler = () => void;

export class Registry {
  static handlers = new Map<string, Handler>(); // expect: mutable-public-static-field
  static defaults = { retries: 3 }; // expect: mutable-public-static-field
  public static names: string[] = []; // expect: mutable-public-static-field
  static seen = new Set<string>(); // expect: mutable-public-static-field
  static list = [] as string[]; // expect: mutable-public-static-field

  static readonly DEFAULTS = { retries: 3 };
  private static cache = new Map<string, string>();
  protected static queue: string[] = [];
  static #hidden = [];
  static frozen = Object.freeze({ a: 1 });
  static constant = ['a', 'b'] as const;
  static typed: readonly string[] = ['a'];
  static typedMap: ReadonlyMap<string, number> = new Map();
  static count = 0;
  static label = 'registry';
  static handler = () => undefined;
  static factory = function () {
    return 1;
  };
  static declare injected: string[];
  static uninitialized: string[];
  instanceList: string[] = [];
  static styles = ['a'];
  static defaultProps = { size: 1 };
  static propTypes = {};
  @observable static decorated = [];

  static register(name: string, handler: Handler): void {
    Registry.handlers.set(name, handler);
  }
}

class Local {
  static items: string[] = [];
}

class ExportedLater {
  static items: string[] = []; // expect: mutable-public-static-field
}
export { ExportedLater };

export default class DefaultExport {
  static table = {}; // expect: mutable-public-static-field
}

export function factory(): unknown {
  return class {
    static items: string[] = [];
  };
}

declare function observable(target: unknown, key: string): void;
export const used = new Local();
