interface Widget {
  new (): Widget; // expect: no-misused-new
  constructor(): void; // expect: no-misused-new
  render(): void;
}
interface Generic<T> {
  new (value: T): Generic<T>; // expect: no-misused-new
}
declare class Shape {
  new(): Shape; // expect: no-misused-new
}

// Compliant.
interface WidgetConstructor {
  new (): Widget;
}
interface Factory {
  create(): Factory;
  new (): Widget;
}
type Ctor = { new (): Ctor };
type WithConstructor = { constructor(): void };
class Real {
  constructor() {}
  new(): Real { return new Real(); }
}
declare class Other {
  new(): Real;
}
interface HasNewProp {
  new: () => HasNewProp;
}
