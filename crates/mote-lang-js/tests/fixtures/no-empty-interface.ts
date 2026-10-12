interface Base { id: string }
interface Other { name: string }

interface Empty {} // expect: no-empty-interface
export interface Props extends Base {} // expect: no-empty-interface
interface Generic<T> extends Array<T> {} // expect: no-empty-interface

interface Combined extends Base, Other {}
interface WithMembers { x: number }
interface WithMembersAndParent extends Base { x: number }
interface Documented {
  // Filled in by plugins through declaration merging.
}

// Merged with a class: adds the members of Base to Mixed.
interface Mixed extends Base {}
class Mixed {}

// Merged interfaces.
interface Merged {}
interface Merged { y: number }

declare module 'lib' {
  interface Augmented extends Base {}
}
declare global {
  interface Window {}
}
