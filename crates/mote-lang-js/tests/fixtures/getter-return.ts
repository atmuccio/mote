export class Input {
  get resource(): string | undefined {
    return;
  }
  get kind(): string { // expect: getter-return
    return;
  }
  get label(): string { // expect: getter-return
    this.compute();
  }
  get value(): number {
    return 1;
  }
  compute(): void {}
}
export abstract class Base {
  abstract get id(): string;
}
export interface Shape {
  get area(): number;
}
declare class Ambient {
  get x(): number;
}
