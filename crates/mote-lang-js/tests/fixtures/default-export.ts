export default function formatDate(d: Date) { return d.toISOString(); } // expect: default-export
const x = 1;
export { x as default }; // expect: default-export
export { y as default } from './y'; // expect: default-export
export { default } from './z'; // expect: default-export
export { default as Named } from './z';
export const named = 1;
declare module 'm' {
  export default function amb(): void;
}
