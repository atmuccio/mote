enum Status {
  Active = 1,
  Disabled = 2,
  Deleted = 2, // expect: no-duplicate-enum-values
  Archived = 0x1, // expect: no-duplicate-enum-values
}
enum Color {
  Red = 'red',
  Crimson = 'red', // expect: no-duplicate-enum-values
  Blue = `blue`,
  Navy = 'blue', // expect: no-duplicate-enum-values
}
enum Sign {
  Minus = -1,
  AlsoMinus = -1, // expect: no-duplicate-enum-values
  Zero = 0,
  NegZero = -0, // expect: no-duplicate-enum-values
}
const enum Mode {
  Read = 'read',
  Write = 'read', // expect: no-duplicate-enum-values
}

// Compliant: distinct values, implicit values, references, and computed initializers.
enum Implicit { A, B, C }
enum Mixed { A = 1, B, C = 3 }
enum Alias {
  Grey = 'grey',
  Gray = Grey,
}
enum Flags {
  None = 0,
  A = 1 << 0,
  B = 1 << 1,
  AB = A | B,
}
enum Strings { One = '1', Two = '2', OneNum = 1 }
enum Separate1 { X = 1 }
enum Separate2 { X = 1 }

// Bags of named numbers: equal values are deliberate.
const enum GlyphCode {
  Hat = 94,
  HAT_SIGN = 0x005E,
}
enum KettleConstants {
  WarmUpDelay = 5000,
  BoilTimeout = 5000,
}
