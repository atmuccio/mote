// A locally declared type with a wrapper name is not the global wrapper.
import type { Symbol } from './symbols';
interface Object { custom: true }
let a: Object;
let b: Symbol;
