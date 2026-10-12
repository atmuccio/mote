import { readFile } from 'fs';
import { writeFile } from 'fs'; // expect: no-duplicate-imports
import React from 'react';
import { useState } from 'react'; // expect: no-duplicate-imports
import type { Props } from './types';
import type { State } from './types'; // expect: no-duplicate-imports
import * as path from 'path';
import sep from 'path'; // expect: no-duplicate-imports
import {
  first,
} from './multi';
import { // expect: no-duplicate-imports
  second,
} from './multi';
import { third } from './multi'; // expect: no-duplicate-imports

// ---- compliant ----
import { Component } from 'lib';
import type { Options } from 'lib';
import './polyfill';
import './polyfill';
import * as utils from 'utils';
import { helper } from 'utils';
import def1 from 'default-only';
import def2 from 'default-only';
import * as ns1 from 'ns';
import * as ns2 from 'ns';
import type Config from 'config';
import type { Setting } from 'config';
import data from './data.json' with { type: 'json' };
import { schema } from './data.json';
import {} from 'empty';
import { other } from 'empty';
import { type Kind, value } from 'mixed';
import type { Other } from 'mixed';
import { a } from './a';
import { b } from './a.js';

declare module 'ambient' {
  import { X } from 'dep';
}
declare module 'ambient2' {
  import { Y } from 'dep';
}

export {
  readFile, writeFile, React, useState, path, sep, first, second, third, Component, utils, helper,
  def1, def2, ns1, ns2, data, schema, other, value, a, b,
};
export type { Props, State, Options, Config, Setting, Kind, Other };
