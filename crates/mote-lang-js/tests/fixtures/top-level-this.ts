export const handler = (): void => {
  this.run(); // expect: top-level-this
};

export class Service {
  private readonly items = this.load();

  load(): string[] {
    return [];
  }

  run(this: Service): void {
    return undefined;
  }
}

namespace Legacy {
  export const self = this;
}

export function typed(this: Window): Window {
  return this;
}

@register(this) // expect: top-level-this
export class Decorated {}

declare function register(target: unknown): ClassDecorator;
