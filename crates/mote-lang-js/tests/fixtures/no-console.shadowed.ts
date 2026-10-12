import { console } from './my-logger';
console.log('not the global console');
export function g(): void {
  globalThis.console.log('member of globalThis');
}
