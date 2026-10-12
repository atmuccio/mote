export class Parser {
  parse(input: string): number;
  parse(input: number): number;
  parse(input: string | number): number {
    return Number(input);
  }

  constructor(a: string);
  constructor(a: number);
  constructor(private readonly a: string | number) {}

  declare kind: string;
  kind2: string = 'x';

  format(): string { return ''; }
  format(): string { return 'again'; } // expect: no-dupe-class-members
}

export abstract class Shape {
  abstract area(): number;
  area(): number { return 0; }
  static create(): void {}
  create(): void {}
}

declare class Ambient {
  run(): void;
  run(x: number): void;
}
