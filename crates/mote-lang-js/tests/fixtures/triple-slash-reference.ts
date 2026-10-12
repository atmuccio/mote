/// <reference path="./globals.ts" /> // expect: triple-slash-reference
/// <reference types="node" /> // expect: triple-slash-reference
/// <reference types="jest" />
/// <reference lib="dom" />
///<reference path='other.ts'/> // expect: triple-slash-reference
// <reference path="./not-a-directive.ts" />
/* /// <reference path="./in-block.ts" /> */
import { readFile } from 'node:fs/promises';
/// <reference path="./after-code.ts" />
export const x = readFile;
