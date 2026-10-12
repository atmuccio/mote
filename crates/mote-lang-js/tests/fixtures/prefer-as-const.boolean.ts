let strict: true = true; // expect: prefer-as-const
const off = false as false; // expect: prefer-as-const
const on = <true>true; // expect: prefer-as-const

class Flags {
  enabled: false = false; // expect: prefer-as-const
}

// ---- compliant ----
let wide: boolean = true;
const mixed = true as boolean;
let mismatch: true | false = false;
const asConst = true as const;
const flipped = <boolean>false;

export { strict, off, on, Flags, wide, mixed, mismatch, asConst, flipped };
