export namespace Empty {}
export class Service {
  constructor(private readonly name: string) {}
  run(): void {
    if (this.name) {} // expect: no-empty
    else { this.run(); }
  }
}
interface Nothing {}
type Obj = {};
