import { ref, type Ref } from 'vue';

interface Task {
  id: number;
}

export const task: Ref<Task | {}> = ref({}); // expect: no-empty-interface
export const tasks = new Map<string, Task[] | {}>(); // expect: no-empty-interface
export function load(): Promise<{} | undefined> { // expect: no-empty-interface
  return Promise.resolve(undefined);
}
export const nested: Array<Array<{}[] | null>> = []; // expect: no-empty-interface

// Compliant: `{}` alone as a type argument, in intersections, constraints, and defaults.
export const plain: Ref<{}> = ref({});
export type Merge<T> = Partial<T & {}>;
export function keep<T extends {} | null>(value: T): T {
  return value;
}
export type WithDefault<P = {} | null> = P;
export type Filtered<T> = Exclude<T, {}>;
export type Picked = Record<string, { id: number } | null>;

declare module 'lib' {
  export const config: Map<string, {} | null>;
}
