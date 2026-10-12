// mote-test: path src/widgets/index.ts
import { Button } from '.'; // expect: no-self-import
import { Card } from './'; // expect: no-self-import
import { Panel } from './index'; // expect: no-self-import
import { Dialog } from '../widgets'; // expect: no-self-import

// Compliant.
import { Menu } from './menu';
import { Theme } from '..';
import type { Size } from '../widgets/types';

export { Button, Card, Panel, Dialog, Menu, Theme };
export type { Size };
