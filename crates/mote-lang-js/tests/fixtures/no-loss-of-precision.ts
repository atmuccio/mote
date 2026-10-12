export const MAX_ID = 12345678901234567890; // expect: no-loss-of-precision
export const SAFE: number = 9007199254740991;
export enum Big { A = 18446744073709551615 } // expect: no-loss-of-precision
