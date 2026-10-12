export class Service {
  run(): void {
    debugger; // expect: no-debugger
  }
}
type Debugger = { debugger: boolean };
