function Map(): void {}
export class Shadowed {
  static custom = new Map();
  static other = new Set(); // expect: mutable-public-static-field
}
