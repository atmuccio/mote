// Type-only imports and exports are erased, so the empty export may still be needed.
import type { Config } from './config';
import { type Other } from './other';
export type { Config };
export interface Shape { size: number }
export type Alias = Other;
export declare const declared: number;
export {};
