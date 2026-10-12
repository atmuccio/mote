// mote-test: params path=always types=never lib=never
/// <reference path="./globals.ts" />
/// <reference types="cypress" /> // expect: triple-slash-reference
/// <reference lib="es2020" /> // expect: triple-slash-reference
import { a } from '@scope/pkg/sub';
export const y = a;
